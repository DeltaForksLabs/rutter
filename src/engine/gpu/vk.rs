// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::ffi::{CStr, CString};
use std::rc::Rc;

use ash::vk::Handle;
use ash::{Device, Entry, Instance, khr, vk as avk};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use skia_safe::{
    Canvas, ColorType, Surface as SkiaSurface,
    gpu::{
        self, DirectContext, FlushInfo, SurfaceOrigin, SyncCpu, backend_render_targets,
        direct_contexts, surfaces,
        vk::{self as skvk, mutable_texture_states},
    },
};
use winit::{
    dpi::PhysicalSize,
    event_loop::ActiveEventLoop,
    window::{Window, WindowAttributes},
};

use super::{BackendFailure, BackendType, GraphicsBackend, GraphicsError};

struct SwapchainImage {
    image: avk::Image,
    render_target: gpu::BackendRenderTarget,
    surface: SkiaSurface,
}

struct SwapchainBundle {
    swapchain: avk::SwapchainKHR,
    extent: avk::Extent2D,
    format: avk::Format,
    color_type: ColorType,
    images: Vec<SwapchainImage>,
    image_layouts: Vec<avk::ImageLayout>,
}

struct PendingSwapchain<'a> {
    loader: &'a khr::swapchain::Device,
    handle: avk::SwapchainKHR,
}

impl<'a> PendingSwapchain<'a> {
    fn commit(mut self) -> avk::SwapchainKHR {
        let handle = self.handle;
        self.handle = avk::SwapchainKHR::null();
        handle
    }
}

impl Drop for PendingSwapchain<'_> {
    fn drop(&mut self) {
        if self.handle.is_null() {
            return;
        }
        unsafe {
            self.loader.destroy_swapchain(self.handle, None);
        }
    }
}

struct VkInitGuard {
    entry: Option<Entry>,
    instance: Option<Instance>,
    surface_loader: Option<khr::surface::Instance>,
    surface: Option<avk::SurfaceKHR>,
    device: Option<Device>,
    command_pool: Option<avk::CommandPool>,
    acquire_fence: Option<avk::Fence>,
    skia_context: Option<DirectContext>,
}

impl VkInitGuard {
    fn new(entry: Entry) -> Self {
        Self {
            entry: Some(entry),
            instance: None,
            surface_loader: None,
            surface: None,
            device: None,
            command_pool: None,
            acquire_fence: None,
            skia_context: None,
        }
    }
}

impl Drop for VkInitGuard {
    fn drop(&mut self) {
        if let Some(device) = self.device.as_ref() {
            unsafe {
                let _ = device.device_wait_idle();
            }
        }
        if let Some(mut context) = self.skia_context.take() {
            context.release_resources_and_abandon();
        }
        if let Some(device) = self.device.as_ref() {
            unsafe {
                if let Some(fence) = self.acquire_fence.take() {
                    device.destroy_fence(fence, None);
                }
                if let Some(pool) = self.command_pool.take() {
                    device.destroy_command_pool(pool, None);
                }
                device.destroy_device(None);
            }
        }
        if let (Some(loader), Some(surface)) = (self.surface_loader.as_ref(), self.surface.take()) {
            unsafe {
                loader.destroy_surface(surface, None);
            }
        }
        if let Some(instance) = self.instance.take() {
            unsafe {
                instance.destroy_instance(None);
            }
        }
    }
}

pub struct VkBackend {
    window: Rc<Window>,
    entry: Entry,
    instance: Instance,
    surface_loader: khr::surface::Instance,
    surface: avk::SurfaceKHR,
    physical_device: avk::PhysicalDevice,
    device: Device,
    queue_family_index: u32,
    queue: avk::Queue,
    swapchain_loader: khr::swapchain::Device,
    swapchain: avk::SwapchainKHR,
    swapchain_format: avk::Format,
    color_type: ColorType,
    extent: avk::Extent2D,
    command_pool: avk::CommandPool,
    transition_command_buffer: avk::CommandBuffer,
    acquire_fence: avk::Fence,
    skia_context: DirectContext,
    swapchain_images: Vec<SwapchainImage>,
    image_layouts: Vec<avk::ImageLayout>,
    current_image_index: Option<u32>,
    transparent: bool,
}

impl VkBackend {
    pub fn try_new(
        event_loop: &ActiveEventLoop,
        attrs: WindowAttributes,
    ) -> Result<Box<dyn GraphicsBackend>, BackendFailure> {
        let transparent = attrs.transparent();
        let window = Rc::new(
            event_loop
                .create_window(attrs)
                .map_err(|err| Self::init_failure(err.to_string()))?,
        );

        let entry = unsafe { Entry::load() }.map_err(|err| Self::init_failure(err.to_string()))?;
        let display_handle = window
            .display_handle()
            .map_err(|err| Self::init_failure(err.to_string()))?;
        let window_handle = window
            .window_handle()
            .map_err(|err| Self::init_failure(err.to_string()))?;

        let mut instance_extension_names =
            ash_window::enumerate_required_extensions(display_handle.as_raw())
                .map_err(|err| Self::init_failure(format!("required surface extensions: {err:?}")))?
                .to_vec();
        let available_instance_extensions = unsafe {
            entry
                .enumerate_instance_extension_properties(None)
                .map_err(|err| {
                    Self::init_failure(format!("enumerate instance extensions: {err:?}"))
                })?
        };
        let portability_supported = supports_extension(
            &available_instance_extensions,
            avk::KHR_PORTABILITY_ENUMERATION_NAME,
        );
        if portability_supported
            && !instance_extension_names
                .iter()
                .any(|ext| *ext == avk::KHR_PORTABILITY_ENUMERATION_NAME.as_ptr())
        {
            instance_extension_names.push(avk::KHR_PORTABILITY_ENUMERATION_NAME.as_ptr());
        }

        let app_name = CString::new("Rutter").unwrap();
        let engine_name = CString::new("Rutter").unwrap();
        let app_info = avk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(avk::make_api_version(0, 0, 5, 1))
            .engine_name(&engine_name)
            .engine_version(avk::make_api_version(0, 0, 5, 1))
            .api_version(avk::API_VERSION_1_0);

        let mut instance_create_flags = avk::InstanceCreateFlags::empty();
        if portability_supported {
            instance_create_flags |= avk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
        }
        let instance_create_info = avk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&instance_extension_names)
            .flags(instance_create_flags);
        let mut cleanup = VkInitGuard::new(entry);
        let instance = unsafe {
            cleanup
                .entry
                .as_ref()
                .unwrap()
                .create_instance(&instance_create_info, None)
        }
        .map_err(|err| Self::init_failure(format!("create Vulkan instance: {err:?}")))?;
        cleanup.instance = Some(instance);
        let instance = cleanup.instance.as_ref().unwrap();

        let surface_loader = khr::surface::Instance::new(cleanup.entry.as_ref().unwrap(), instance);
        cleanup.surface_loader = Some(surface_loader);
        let surface_loader = cleanup.surface_loader.as_ref().unwrap();
        let surface = unsafe {
            ash_window::create_surface(
                cleanup.entry.as_ref().unwrap(),
                instance,
                display_handle.as_raw(),
                window_handle.as_raw(),
                None,
            )
        }
        .map_err(|err| Self::init_failure(format!("create Vulkan surface: {err:?}")))?;
        cleanup.surface = Some(surface);

        let (physical_device, queue_family_index) =
            pick_physical_device(&instance, &surface_loader, surface, transparent)
                .map_err(Self::init_failure)?;

        let device_extensions =
            device_extension_names(&instance, physical_device).map_err(Self::init_failure)?;
        let queue_priorities = [1.0_f32];
        let queue_create_info = avk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&queue_priorities);
        let device_create_info = avk::DeviceCreateInfo::default()
            .queue_create_infos(std::slice::from_ref(&queue_create_info))
            .enabled_extension_names(&device_extensions);
        let device = unsafe { instance.create_device(physical_device, &device_create_info, None) }
            .map_err(|err| Self::init_failure(format!("create Vulkan device: {err:?}")))?;
        cleanup.device = Some(device);
        let device = cleanup.device.as_ref().unwrap();
        let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

        let swapchain_loader = khr::swapchain::Device::new(&instance, &device);
        let command_pool_info = avk::CommandPoolCreateInfo::default()
            .flags(avk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(queue_family_index);
        let command_pool = unsafe { device.create_command_pool(&command_pool_info, None) }
            .map_err(|err| Self::init_failure(format!("create command pool: {err:?}")))?;
        cleanup.command_pool = Some(command_pool);
        let command_buffer_info = avk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(avk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let transition_command_buffer =
            unsafe { device.allocate_command_buffers(&command_buffer_info) }
                .map_err(|err| Self::init_failure(format!("allocate command buffer: {err:?}")))?
                .into_iter()
                .next()
                .ok_or_else(|| Self::init_failure("allocate command buffer returned empty"))?;
        let acquire_fence = unsafe { device.create_fence(&avk::FenceCreateInfo::default(), None) }
            .map_err(|err| Self::init_failure(format!("create acquire fence: {err:?}")))?;
        cleanup.acquire_fence = Some(acquire_fence);

        let skia_context = create_skia_context(
            cleanup.entry.as_ref().unwrap(),
            &instance,
            physical_device,
            &device,
            queue,
            queue_family_index,
        )
        .map_err(Self::init_failure)?;
        cleanup.skia_context = Some(skia_context);
        let skia_context = cleanup.skia_context.as_mut().unwrap();

        let swapchain_bundle = Self::create_swapchain_bundle(
            &window,
            &surface_loader,
            surface,
            physical_device,
            &swapchain_loader,
            skia_context,
            queue_family_index,
            transparent,
            None,
        )
        .map_err(Self::init_failure)?;

        let entry = cleanup.entry.take().unwrap();
        let instance = cleanup.instance.take().unwrap();
        let surface_loader = cleanup.surface_loader.take().unwrap();
        let surface = cleanup.surface.take().unwrap();
        let device = cleanup.device.take().unwrap();
        let command_pool = cleanup.command_pool.take().unwrap();
        let acquire_fence = cleanup.acquire_fence.take().unwrap();
        let skia_context = cleanup.skia_context.take().unwrap();
        Ok(Box::new(Self {
            window,
            entry,
            instance,
            surface_loader,
            surface,
            physical_device,
            device,
            queue_family_index,
            queue,
            swapchain_loader,
            swapchain: swapchain_bundle.swapchain,
            swapchain_format: swapchain_bundle.format,
            color_type: swapchain_bundle.color_type,
            extent: swapchain_bundle.extent,
            command_pool,
            transition_command_buffer,
            acquire_fence,
            skia_context,
            swapchain_images: swapchain_bundle.images,
            image_layouts: swapchain_bundle.image_layouts,
            current_image_index: None,
            transparent,
        }))
    }

    fn init_failure(reason: impl Into<String>) -> BackendFailure {
        BackendFailure::new(BackendType::Vulkan, reason)
    }

    fn frame_error(reason: impl Into<String>) -> GraphicsError {
        GraphicsError::Frame {
            backend: BackendType::Vulkan,
            reason: reason.into(),
        }
    }

    fn resize_error(reason: impl Into<String>) -> GraphicsError {
        GraphicsError::Resize {
            backend: BackendType::Vulkan,
            reason: reason.into(),
        }
    }

    fn create_swapchain_bundle(
        window: &Window,
        surface_loader: &khr::surface::Instance,
        surface: avk::SurfaceKHR,
        physical_device: avk::PhysicalDevice,
        swapchain_loader: &khr::swapchain::Device,
        skia_context: &mut DirectContext,
        queue_family_index: u32,
        transparent: bool,
        old_swapchain: Option<avk::SwapchainKHR>,
    ) -> Result<SwapchainBundle, String> {
        let capabilities = unsafe {
            surface_loader
                .get_physical_device_surface_capabilities(physical_device, surface)
                .map_err(|err| format!("query surface capabilities: {err:?}"))?
        };
        let formats = unsafe {
            surface_loader
                .get_physical_device_surface_formats(physical_device, surface)
                .map_err(|err| format!("query surface formats: {err:?}"))?
        };
        if formats.is_empty() {
            return Err("surface reported no swapchain formats".to_string());
        }
        let present_modes = unsafe {
            surface_loader
                .get_physical_device_surface_present_modes(physical_device, surface)
                .map_err(|err| format!("query present modes: {err:?}"))?
        };

        let (format, color_space, color_type) = pick_surface_format(&formats)?;
        let extent = choose_extent(window.inner_size(), capabilities);
        let min_image_count = choose_image_count(capabilities);
        let composite_alpha =
            pick_composite_alpha(capabilities.supported_composite_alpha, transparent)?;
        let present_mode = pick_present_mode(&present_modes);

        let swapchain_info = avk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(min_image_count)
            .image_format(format)
            .image_color_space(color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(avk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(avk::SharingMode::EXCLUSIVE)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(composite_alpha)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(old_swapchain.unwrap_or_default());
        let swapchain = unsafe { swapchain_loader.create_swapchain(&swapchain_info, None) }
            .map_err(|err| format!("create swapchain: {err:?}"))?;
        let pending_swapchain = PendingSwapchain {
            loader: swapchain_loader,
            handle: swapchain,
        };
        let images = unsafe { swapchain_loader.get_swapchain_images(pending_swapchain.handle) }
            .map_err(|err| format!("get swapchain images: {err:?}"))?;
        if images.is_empty() {
            return Err("swapchain returned no images".to_string());
        }

        let mut swapchain_images = Vec::with_capacity(images.len());
        for image in images {
            let swapchain_image = create_swapchain_image(
                image,
                extent,
                format,
                color_type,
                queue_family_index,
                skia_context,
            )?;
            swapchain_images.push(swapchain_image);
        }

        Ok(SwapchainBundle {
            swapchain: pending_swapchain.commit(),
            extent,
            format,
            color_type,
            image_layouts: vec![avk::ImageLayout::UNDEFINED; swapchain_images.len()],
            images: swapchain_images,
        })
    }

    fn recreate_swapchain(&mut self, size: PhysicalSize<u32>) -> Result<(), GraphicsError> {
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }

        unsafe {
            self.device
                .device_wait_idle()
                .map_err(|err| Self::resize_error(format!("wait device idle: {err:?}")))?;
        }

        let old_swapchain = self.swapchain;
        let old_images = std::mem::take(&mut self.swapchain_images);
        let old_layouts = std::mem::take(&mut self.image_layouts);
        let old_format = self.swapchain_format;
        let old_color_type = self.color_type;
        let old_extent = self.extent;

        let bundle = match Self::create_swapchain_bundle(
            &self.window,
            &self.surface_loader,
            self.surface,
            self.physical_device,
            &self.swapchain_loader,
            &mut self.skia_context,
            self.queue_family_index,
            self.transparent,
            Some(old_swapchain),
        ) {
            Ok(bundle) => bundle,
            Err(reason) => {
                self.swapchain_images = old_images;
                self.image_layouts = old_layouts;
                self.swapchain = old_swapchain;
                self.swapchain_format = old_format;
                self.color_type = old_color_type;
                self.extent = old_extent;
                return Err(Self::resize_error(reason));
            }
        };

        self.swapchain = bundle.swapchain;
        self.swapchain_format = bundle.format;
        self.color_type = bundle.color_type;
        self.extent = bundle.extent;
        self.swapchain_images = bundle.images;
        self.image_layouts = bundle.image_layouts;
        self.current_image_index = None;

        drop(old_images);
        unsafe {
            self.swapchain_loader.destroy_swapchain(old_swapchain, None);
        }

        Ok(())
    }

    fn acquire_next_image_index(&mut self) -> Result<u32, GraphicsError> {
        unsafe {
            self.device
                .reset_fences(std::slice::from_ref(&self.acquire_fence))
                .map_err(|err| Self::frame_error(format!("reset acquire fence: {err:?}")))?;
        }

        let acquire = unsafe {
            self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                avk::Semaphore::null(),
                self.acquire_fence,
            )
        };

        let (image_index, _suboptimal) = match acquire {
            Ok(result) => result,
            Err(avk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.recreate_swapchain(self.window.inner_size())?;
                unsafe {
                    self.device
                        .reset_fences(std::slice::from_ref(&self.acquire_fence))
                        .map_err(|err| {
                            Self::frame_error(format!("reset acquire fence after resize: {err:?}"))
                        })?;
                }
                unsafe {
                    self.swapchain_loader.acquire_next_image(
                        self.swapchain,
                        u64::MAX,
                        avk::Semaphore::null(),
                        self.acquire_fence,
                    )
                }
                .map_err(|err| Self::frame_error(format!("acquire swapchain image: {err:?}")))?
            }
            Err(err) => {
                return Err(Self::frame_error(format!(
                    "acquire swapchain image: {err:?}"
                )));
            }
        };

        unsafe {
            self.device
                .wait_for_fences(std::slice::from_ref(&self.acquire_fence), true, u64::MAX)
                .map_err(|err| Self::frame_error(format!("wait acquire fence: {err:?}")))?;
        }

        Ok(image_index)
    }

    fn transition_swapchain_image(
        &mut self,
        image_index: u32,
        new_layout: avk::ImageLayout,
    ) -> Result<(), GraphicsError> {
        let old_layout = self.image_layouts[image_index as usize];
        if old_layout == new_layout {
            return Ok(());
        }

        let image = self.swapchain_images[image_index as usize].image;
        let (src_stage, src_access) = layout_barrier_source(old_layout);
        let (dst_stage, dst_access) = layout_barrier_destination(new_layout);
        let subresource_range = avk::ImageSubresourceRange {
            aspect_mask: avk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        };
        let barrier = avk::ImageMemoryBarrier::default()
            .old_layout(old_layout)
            .new_layout(new_layout)
            .src_access_mask(src_access)
            .dst_access_mask(dst_access)
            .src_queue_family_index(avk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(avk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(subresource_range);
        let begin_info = avk::CommandBufferBeginInfo::default()
            .flags(avk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe {
            self.device
                .reset_command_buffer(
                    self.transition_command_buffer,
                    avk::CommandBufferResetFlags::empty(),
                )
                .map_err(|err| {
                    Self::frame_error(format!("reset transition command buffer: {err:?}"))
                })?;
            self.device
                .begin_command_buffer(self.transition_command_buffer, &begin_info)
                .map_err(|err| {
                    Self::frame_error(format!("begin transition command buffer: {err:?}"))
                })?;
            self.device.cmd_pipeline_barrier(
                self.transition_command_buffer,
                src_stage,
                dst_stage,
                avk::DependencyFlags::empty(),
                &[],
                &[],
                std::slice::from_ref(&barrier),
            );
            self.device
                .end_command_buffer(self.transition_command_buffer)
                .map_err(|err| {
                    Self::frame_error(format!("end transition command buffer: {err:?}"))
                })?;
            let submit_info = avk::SubmitInfo::default()
                .command_buffers(std::slice::from_ref(&self.transition_command_buffer));
            self.device
                .queue_submit(
                    self.queue,
                    std::slice::from_ref(&submit_info),
                    avk::Fence::null(),
                )
                .map_err(|err| Self::frame_error(format!("submit layout transition: {err:?}")))?;
            self.device.queue_wait_idle(self.queue).map_err(|err| {
                Self::frame_error(format!("wait queue idle after transition: {err:?}"))
            })?;
        }

        self.image_layouts[image_index as usize] = new_layout;
        let swapchain_image = &mut self.swapchain_images[image_index as usize];
        backend_render_targets::set_vk_image_layout(
            &mut swapchain_image.render_target,
            map_skia_image_layout(new_layout),
        );

        Ok(())
    }
}

impl GraphicsBackend for VkBackend {
    fn backend_type(&self) -> BackendType {
        BackendType::Vulkan
    }

    fn begin_frame(&mut self) -> Result<&Canvas, GraphicsError> {
        if self.swapchain_images.is_empty() {
            return Err(Self::frame_error("swapchain has no images"));
        }

        let image_index = self.acquire_next_image_index()?;
        self.transition_swapchain_image(image_index, avk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)?;

        let swapchain_image = &mut self.swapchain_images[image_index as usize];
        let color_attachment_state = mutable_texture_states::new_vulkan(
            skvk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            self.queue_family_index,
        );
        self.skia_context.set_backend_render_target_state(
            &swapchain_image.render_target,
            &color_attachment_state,
        );
        backend_render_targets::set_vk_image_layout(
            &mut swapchain_image.render_target,
            skvk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        );

        self.current_image_index = Some(image_index);
        Ok(swapchain_image.surface.canvas())
    }

    fn end_frame(&mut self) -> Result<(), GraphicsError> {
        let image_index = self.current_image_index.take().ok_or_else(|| {
            Self::frame_error("end_frame called without an active swapchain image")
        })?;

        {
            let swapchain_image = &mut self.swapchain_images[image_index as usize];
            let present_state = mutable_texture_states::new_vulkan(
                skvk::ImageLayout::PRESENT_SRC_KHR,
                self.queue_family_index,
            );
            self.skia_context.flush_surface_with_texture_state(
                &mut swapchain_image.surface,
                &FlushInfo::default(),
                Some(&present_state),
            );
            self.skia_context.submit(SyncCpu::Yes);
            backend_render_targets::set_vk_image_layout(
                &mut swapchain_image.render_target,
                skvk::ImageLayout::PRESENT_SRC_KHR,
            );
        }

        unsafe {
            self.device.queue_wait_idle(self.queue).map_err(|err| {
                Self::frame_error(format!("wait queue idle before present: {err:?}"))
            })?;
        }
        self.image_layouts[image_index as usize] = avk::ImageLayout::PRESENT_SRC_KHR;

        let swapchains = [self.swapchain];
        let image_indices = [image_index];
        let present_info = avk::PresentInfoKHR::default()
            .swapchains(&swapchains)
            .image_indices(&image_indices);
        match unsafe {
            self.swapchain_loader
                .queue_present(self.queue, &present_info)
        } {
            Ok(suboptimal) => {
                if suboptimal {
                    self.recreate_swapchain(self.window.inner_size())?;
                }
            }
            Err(avk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.recreate_swapchain(self.window.inner_size())?;
            }
            Err(err) => {
                return Err(Self::frame_error(format!(
                    "present swapchain image: {err:?}"
                )));
            }
        }

        Ok(())
    }

    fn resize(&mut self, size: PhysicalSize<u32>) -> Result<(), GraphicsError> {
        self.recreate_swapchain(size)
    }

    fn window(&self) -> &Rc<Window> {
        &self.window
    }

    fn skia_context(&mut self) -> Option<&mut DirectContext> {
        Some(&mut self.skia_context)
    }
}

impl Drop for VkBackend {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
        }
        self.current_image_index = None;
        self.swapchain_images.clear();
        self.skia_context.release_resources_and_abandon();
        unsafe {
            self.device.destroy_fence(self.acquire_fence, None);
            self.device.destroy_command_pool(self.command_pool, None);
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
        let _ = &self.entry;
    }
}

fn pick_physical_device(
    instance: &Instance,
    surface_loader: &khr::surface::Instance,
    surface: avk::SurfaceKHR,
    transparent: bool,
) -> Result<(avk::PhysicalDevice, u32), String> {
    let physical_devices = unsafe {
        instance
            .enumerate_physical_devices()
            .map_err(|err| format!("enumerate physical devices: {err:?}"))?
    };
    if physical_devices.is_empty() {
        return Err("no Vulkan physical devices available".to_string());
    }

    for physical_device in physical_devices {
        if !device_supports_swapchain(instance, physical_device)? {
            continue;
        }
        if transparent
            && !device_supports_transparent_surface(surface_loader, physical_device, surface)?
        {
            continue;
        }

        let queue_families =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
        for (index, family) in queue_families.iter().enumerate() {
            if !family.queue_flags.contains(avk::QueueFlags::GRAPHICS) {
                continue;
            }

            let supports_present = unsafe {
                surface_loader
                    .get_physical_device_surface_support(physical_device, index as u32, surface)
                    .map_err(|err| format!("query surface support: {err:?}"))?
            };
            if supports_present {
                return Ok((physical_device, index as u32));
            }
        }
    }

    Err("no Vulkan device satisfies graphics, presentation, and surface alpha requirements".into())
}

fn device_supports_transparent_surface(
    surface_loader: &khr::surface::Instance,
    physical_device: avk::PhysicalDevice,
    surface: avk::SurfaceKHR,
) -> Result<bool, String> {
    let capabilities = unsafe {
        surface_loader
            .get_physical_device_surface_capabilities(physical_device, surface)
            .map_err(|err| format!("query transparent surface capabilities: {err:?}"))?
    };
    Ok(capabilities
        .supported_composite_alpha
        .contains(avk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED))
}

fn device_extension_names(
    instance: &Instance,
    physical_device: avk::PhysicalDevice,
) -> Result<Vec<*const i8>, String> {
    let available_extensions = unsafe {
        instance
            .enumerate_device_extension_properties(physical_device)
            .map_err(|err| format!("enumerate device extensions: {err:?}"))?
    };
    let mut extensions = vec![avk::KHR_SWAPCHAIN_NAME.as_ptr()];
    if supports_extension(&available_extensions, avk::KHR_PORTABILITY_SUBSET_NAME) {
        extensions.push(avk::KHR_PORTABILITY_SUBSET_NAME.as_ptr());
    }
    Ok(extensions)
}

fn device_supports_swapchain(
    instance: &Instance,
    physical_device: avk::PhysicalDevice,
) -> Result<bool, String> {
    let available_extensions = unsafe {
        instance
            .enumerate_device_extension_properties(physical_device)
            .map_err(|err| format!("enumerate device extensions: {err:?}"))?
    };
    Ok(supports_extension(
        &available_extensions,
        avk::KHR_SWAPCHAIN_NAME,
    ))
}

fn supports_extension(extensions: &[avk::ExtensionProperties], target: &CStr) -> bool {
    extensions.iter().any(|extension| {
        let name = unsafe { CStr::from_ptr(extension.extension_name.as_ptr()) };
        name == target
    })
}

fn pick_surface_format(
    formats: &[avk::SurfaceFormatKHR],
) -> Result<(avk::Format, avk::ColorSpaceKHR, ColorType), String> {
    if formats.len() == 1 && formats[0].format == avk::Format::UNDEFINED {
        return Ok((
            avk::Format::B8G8R8A8_UNORM,
            formats[0].color_space,
            ColorType::BGRA8888,
        ));
    }

    for preferred in [
        avk::Format::B8G8R8A8_UNORM,
        avk::Format::R8G8B8A8_UNORM,
        avk::Format::B8G8R8A8_SRGB,
        avk::Format::R8G8B8A8_SRGB,
    ] {
        if let Some(surface_format) = formats.iter().find(|fmt| fmt.format == preferred) {
            let color_type = match preferred {
                avk::Format::B8G8R8A8_UNORM | avk::Format::B8G8R8A8_SRGB => ColorType::BGRA8888,
                avk::Format::R8G8B8A8_UNORM | avk::Format::R8G8B8A8_SRGB => ColorType::RGBA8888,
                _ => unreachable!(),
            };
            return Ok((
                surface_format.format,
                surface_format.color_space,
                color_type,
            ));
        }
    }

    let fallback = formats[0];
    let color_type = match fallback.format {
        avk::Format::B8G8R8A8_UNORM | avk::Format::B8G8R8A8_SRGB => ColorType::BGRA8888,
        avk::Format::R8G8B8A8_UNORM | avk::Format::R8G8B8A8_SRGB => ColorType::RGBA8888,
        other => {
            return Err(format!(
                "unsupported swapchain color format for Skia: {other:?}"
            ));
        }
    };
    Ok((fallback.format, fallback.color_space, color_type))
}

fn choose_extent(
    size: PhysicalSize<u32>,
    capabilities: avk::SurfaceCapabilitiesKHR,
) -> avk::Extent2D {
    if capabilities.current_extent.width != u32::MAX {
        return capabilities.current_extent;
    }

    avk::Extent2D {
        width: size.width.clamp(
            capabilities.min_image_extent.width,
            capabilities.max_image_extent.width,
        ),
        height: size.height.clamp(
            capabilities.min_image_extent.height,
            capabilities.max_image_extent.height,
        ),
    }
}

fn choose_image_count(capabilities: avk::SurfaceCapabilitiesKHR) -> u32 {
    let desired = capabilities.min_image_count.saturating_add(1).max(2);
    if capabilities.max_image_count == 0 {
        desired
    } else {
        desired.min(capabilities.max_image_count)
    }
}

fn pick_composite_alpha(
    supported: avk::CompositeAlphaFlagsKHR,
    transparent: bool,
) -> Result<avk::CompositeAlphaFlagsKHR, String> {
    if transparent {
        if supported.contains(avk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED) {
            return Ok(avk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED);
        }
        return Err(format!(
            "surface supports composite alpha flags {supported:?}; expected PRE_MULTIPLIED for a transparent top-level surface"
        ));
    }
    for alpha in [
        avk::CompositeAlphaFlagsKHR::OPAQUE,
        avk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
        avk::CompositeAlphaFlagsKHR::POST_MULTIPLIED,
        avk::CompositeAlphaFlagsKHR::INHERIT,
    ] {
        if supported.contains(alpha) {
            return Ok(alpha);
        }
    }
    Err("surface supports no composite alpha mode".to_string())
}

fn pick_present_mode(present_modes: &[avk::PresentModeKHR]) -> avk::PresentModeKHR {
    if present_modes.contains(&avk::PresentModeKHR::MAILBOX) {
        avk::PresentModeKHR::MAILBOX
    } else {
        avk::PresentModeKHR::FIFO
    }
}

fn create_skia_context(
    entry: &Entry,
    instance: &Instance,
    physical_device: avk::PhysicalDevice,
    device: &Device,
    queue: avk::Queue,
    queue_family_index: u32,
) -> Result<DirectContext, String> {
    let get_proc = |proc| match proc {
        skvk::GetProcOf::Instance(instance_handle, name) => unsafe {
            entry
                .get_instance_proc_addr(avk::Instance::from_raw(instance_handle as _), name)
                .map(|func| func as _)
                .unwrap_or(std::ptr::null())
        },
        skvk::GetProcOf::Device(device_handle, name) => unsafe {
            instance
                .get_device_proc_addr(avk::Device::from_raw(device_handle as _), name)
                .map(|func| func as _)
                .unwrap_or(std::ptr::null())
        },
    };

    unsafe {
        direct_contexts::make_vulkan(
            &skvk::BackendContext::new(
                instance.handle().as_raw() as _,
                physical_device.as_raw() as _,
                device.handle().as_raw() as _,
                (queue.as_raw() as _, queue_family_index as usize),
                &get_proc,
            ),
            None,
        )
        .ok_or_else(|| "failed to create Skia Vulkan direct context".to_string())
    }
}

fn create_swapchain_image(
    image: avk::Image,
    extent: avk::Extent2D,
    format: avk::Format,
    color_type: ColorType,
    queue_family_index: u32,
    skia_context: &mut DirectContext,
) -> Result<SwapchainImage, String> {
    let vk_format = map_skia_format(format)?;
    let image_info = unsafe {
        skvk::ImageInfo::new(
            image.as_raw() as _,
            skvk::Alloc::default(),
            skvk::ImageTiling::OPTIMAL,
            skvk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk_format,
            1,
            Some(queue_family_index),
            None,
            None,
            None,
        )
    };

    let render_target =
        backend_render_targets::make_vk((extent.width as i32, extent.height as i32), &image_info);
    let surface = surfaces::wrap_backend_render_target(
        skia_context,
        &render_target,
        SurfaceOrigin::TopLeft,
        color_type,
        None,
        None,
    )
    .ok_or_else(|| "failed to wrap swapchain image with Skia surface".to_string())?;

    Ok(SwapchainImage {
        image,
        render_target,
        surface,
    })
}

fn map_skia_format(format: avk::Format) -> Result<skvk::Format, String> {
    match format {
        avk::Format::B8G8R8A8_UNORM => Ok(skvk::Format::B8G8R8A8_UNORM),
        avk::Format::R8G8B8A8_UNORM => Ok(skvk::Format::R8G8B8A8_UNORM),
        avk::Format::B8G8R8A8_SRGB => Ok(skvk::Format::B8G8R8A8_SRGB),
        avk::Format::R8G8B8A8_SRGB => Ok(skvk::Format::R8G8B8A8_SRGB),
        other => Err(format!("unsupported Vulkan format for Skia: {other:?}")),
    }
}

fn map_skia_image_layout(layout: avk::ImageLayout) -> skvk::ImageLayout {
    match layout {
        avk::ImageLayout::UNDEFINED => skvk::ImageLayout::UNDEFINED,
        avk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL => skvk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        avk::ImageLayout::PRESENT_SRC_KHR => skvk::ImageLayout::PRESENT_SRC_KHR,
        _ => skvk::ImageLayout::GENERAL,
    }
}

fn layout_barrier_source(layout: avk::ImageLayout) -> (avk::PipelineStageFlags, avk::AccessFlags) {
    match layout {
        avk::ImageLayout::UNDEFINED => (
            avk::PipelineStageFlags::TOP_OF_PIPE,
            avk::AccessFlags::empty(),
        ),
        avk::ImageLayout::PRESENT_SRC_KHR => (
            avk::PipelineStageFlags::BOTTOM_OF_PIPE,
            avk::AccessFlags::MEMORY_READ,
        ),
        avk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL => (
            avk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            avk::AccessFlags::COLOR_ATTACHMENT_READ | avk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        ),
        _ => (
            avk::PipelineStageFlags::ALL_COMMANDS,
            avk::AccessFlags::MEMORY_READ | avk::AccessFlags::MEMORY_WRITE,
        ),
    }
}

fn layout_barrier_destination(
    layout: avk::ImageLayout,
) -> (avk::PipelineStageFlags, avk::AccessFlags) {
    match layout {
        avk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL => (
            avk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            avk::AccessFlags::COLOR_ATTACHMENT_READ | avk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        ),
        avk::ImageLayout::PRESENT_SRC_KHR => (
            avk::PipelineStageFlags::BOTTOM_OF_PIPE,
            avk::AccessFlags::MEMORY_READ,
        ),
        _ => (
            avk::PipelineStageFlags::ALL_COMMANDS,
            avk::AccessFlags::MEMORY_READ | avk::AccessFlags::MEMORY_WRITE,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{avk, pick_composite_alpha};

    #[test]
    fn transparent_swapchain_requires_premultiplied_composite_alpha() {
        let supported =
            avk::CompositeAlphaFlagsKHR::OPAQUE | avk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED;

        assert_eq!(
            pick_composite_alpha(supported, true).unwrap(),
            avk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED
        );
        assert!(pick_composite_alpha(avk::CompositeAlphaFlagsKHR::OPAQUE, true).is_err());
    }

    #[test]
    fn opaque_swapchain_preserves_opaque_preference() {
        let supported =
            avk::CompositeAlphaFlagsKHR::OPAQUE | avk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED;

        assert_eq!(
            pick_composite_alpha(supported, false).unwrap(),
            avk::CompositeAlphaFlagsKHR::OPAQUE
        );
    }
}
