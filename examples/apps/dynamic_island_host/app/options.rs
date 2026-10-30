use std::{ffi::OsString, io, path::PathBuf};

use rutter::{
    Layer, LayerAnchors, LayerExclusiveZone, LayerKeyboardInteractivity, LayerShellConfig,
    MessageIngressConfig, SurfaceId, SurfaceInputRegion, SurfaceRequest, WindowConfig,
};

const USAGE: &str = "usage: dynamic_island_host <existing-private-0700-directory> [--overlay]";

#[derive(Debug, PartialEq)]
pub(super) struct Options {
    pub directory: PathBuf,
    pub overlay: bool,
}

pub(super) fn parse(args: impl IntoIterator<Item = OsString>) -> io::Result<Options> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidInput, USAGE);
    let mut args = args.into_iter();
    let directory = args.next().ok_or_else(invalid)?;
    if directory.is_empty()
        || directory.as_encoded_bytes().starts_with(b"-")
        || directory.as_encoded_bytes().contains(&0)
    {
        return Err(invalid());
    }
    let overlay = match args.next() {
        None => false,
        Some(flag) if flag == "--overlay" => true,
        _ => return Err(invalid()),
    };
    if args.next().is_some() {
        return Err(invalid());
    }
    Ok(Options {
        directory: directory.into(),
        overlay,
    })
}

// Resolve every fallible static configuration before acquiring the IPC endpoint.
pub(super) fn configuration(
    overlay: bool,
) -> Result<(SurfaceRequest, MessageIngressConfig), Box<dyn std::error::Error>> {
    let config = WindowConfig::default().with_title("Dynamic Island activity host");
    let config = if overlay {
        let layer = LayerShellConfig::new("rutter-activity-host", LayerAnchors::TOP, 420, 300)?
            .with_layer(Layer::Overlay)
            .with_exclusive_zone(LayerExclusiveZone::None)?
            .with_keyboard_interactivity(LayerKeyboardInteractivity::OnDemand);
        config
            .with_transparent(true)
            .with_decorations(false)
            .with_resizable(false)
            .with_input_region(SurfaceInputRegion::Empty)
            .with_layer_shell(layer)
    } else {
        config.with_inner_size(800, 700)?
    };
    Ok((
        SurfaceRequest::new(SurfaceId::PRIMARY, config),
        MessageIngressConfig::try_new(64, 16)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_os_arguments_and_native_configuration() {
        for args in [
            vec![],
            vec!["--overlay"],
            vec![""],
            vec!["/private\0invalid"],
            vec!["/private", "--unknown"],
            vec!["/private", "--overlay", "--overlay"],
            vec!["/private", "extra"],
        ] {
            assert!(parse(args.into_iter().map(OsString::from)).is_err());
        }
        use std::os::unix::ffi::OsStringExt;
        let path = OsString::from_vec(vec![b'/', 0xff]);
        assert_eq!(
            parse([path.clone()]).unwrap().directory,
            PathBuf::from(path)
        );
        assert!(parse(["/private".into(), OsString::from_vec(vec![0xff])]).is_err());
        for overlay in [false, true] {
            let args = if overlay {
                vec!["/private", "--overlay"]
            } else {
                vec!["/private"]
            };
            assert_eq!(
                parse(args.into_iter().map(OsString::from)).unwrap().overlay,
                overlay
            );
            let (request, _) = configuration(overlay).unwrap();
            let window = request.window;
            assert_eq!(window.layer_shell().is_some(), overlay);
            if overlay {
                assert!(window.is_transparent());
                assert!(!window.has_decorations());
                assert!(!window.is_resizable());
                assert_eq!(window.input_region(), &SurfaceInputRegion::Empty);
                let layer = window.layer_shell().unwrap();
                assert_eq!(layer.layer(), Layer::Overlay);
                assert_eq!(layer.anchors(), LayerAnchors::TOP);
                assert_eq!(layer.logical_size(), (420, 300));
                assert_eq!(layer.exclusive_zone(), LayerExclusiveZone::None);
                assert_eq!(
                    layer.keyboard_interactivity(),
                    LayerKeyboardInteractivity::OnDemand
                );
                assert!(layer.validate_protocol_version(3).is_err());
            }
        }
    }
}
