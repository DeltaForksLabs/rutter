// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

pub(crate) fn encoded_image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    png_dimensions(data)
        .or_else(|| gif_dimensions(data))
        .or_else(|| bmp_dimensions(data))
        .or_else(|| jpeg_dimensions(data))
}

fn png_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if !data.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    Some((
        read_be_u32(data.get(16..20)?)?,
        read_be_u32(data.get(20..24)?)?,
    ))
}

fn gif_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if !data.starts_with(b"GIF87a") && !data.starts_with(b"GIF89a") {
        return None;
    }
    Some((
        read_le_u16(data.get(6..8)?)? as u32,
        read_le_u16(data.get(8..10)?)? as u32,
    ))
}

fn bmp_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if !data.starts_with(b"BM") {
        return None;
    }
    Some((
        read_le_u32(data.get(18..22)?)?,
        read_le_u32(data.get(22..26)?)?,
    ))
}

fn jpeg_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if !data.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let mut offset = 2;
    while offset + 8 < data.len() {
        let marker = *data.get(offset + 1)?;
        let length = usize::from(read_be_u16(data.get(offset + 2..offset + 4)?)?);
        if is_jpeg_start_of_frame(marker) && length >= 7 {
            return Some((
                read_be_u16(data.get(offset + 7..offset + 9)?)? as u32,
                read_be_u16(data.get(offset + 5..offset + 7)?)? as u32,
            ));
        }
        offset = offset.checked_add(length + 2)?;
    }
    None
}

fn is_jpeg_start_of_frame(marker: u8) -> bool {
    matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf)
}

fn read_be_u16(bytes: &[u8]) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.try_into().ok()?))
}

fn read_be_u32(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.try_into().ok()?))
}

fn read_le_u16(bytes: &[u8]) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.try_into().ok()?))
}

fn read_le_u32(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::encoded_image_dimensions;

    #[test]
    fn encoded_image_dimensions_reads_common_raster_headers() {
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x02\0\0\0\x03";
        let gif = b"GIF89a\x04\0\x05\0";
        let bmp = bmp_header(6, 7);
        let jpeg = b"\xff\xd8\xff\xc0\0\x07\x08\0\x08\0\x09";

        assert_eq!(encoded_image_dimensions(png), Some((2, 3)));
        assert_eq!(encoded_image_dimensions(gif), Some((4, 5)));
        assert_eq!(encoded_image_dimensions(&bmp), Some((6, 7)));
        assert_eq!(encoded_image_dimensions(jpeg), Some((9, 8)));
    }

    fn bmp_header(width: u32, height: u32) -> Vec<u8> {
        let mut header = vec![0; 26];
        header[..2].copy_from_slice(b"BM");
        header[18..22].copy_from_slice(&width.to_le_bytes());
        header[22..26].copy_from_slice(&height.to_le_bytes());
        header
    }
}
