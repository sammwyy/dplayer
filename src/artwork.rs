//! Cover art decoding and the soft backdrop derived from it.
//!
//! Runs on a background thread, so it only hands plain pixel buffers back to
//! the UI thread.

use creamui_image::ImageData;

const BACKDROP_SIDE: u32 = 36;
const BLUR_RADIUS: i32 = 3;
const BLUR_PASSES: usize = 3;

pub struct Artwork {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    backdrop: (u32, u32, Vec<u8>),
}

impl Artwork {
    pub fn into_images(self) -> (Option<ImageData>, Option<ImageData>) {
        let (bw, bh, backdrop) = self.backdrop;
        (
            ImageData::from_premultiplied_rgba(self.width, self.height, self.pixels).ok(),
            ImageData::from_premultiplied_rgba(bw, bh, backdrop).ok(),
        )
    }
}

pub fn decode(bytes: &[u8]) -> Option<Artwork> {
    let image = ImageData::from_bytes(bytes).ok()?;
    let (width, height) = (image.width(), image.height());
    let pixels = image.pixels().to_vec();
    let backdrop = blur(downscale(&pixels, width, height));
    Some(Artwork {
        width,
        height,
        pixels,
        backdrop,
    })
}

/// Area-averages premultiplied pixels into a small square thumbnail,
/// center-cropping so the backdrop has the cover's proportions at any size.
fn downscale(pixels: &[u8], width: u32, height: u32) -> (u32, u32, Vec<u8>) {
    let side = width.min(height).max(1);
    let (x0, y0) = ((width - side) / 2, (height - side) / 2);
    let out = BACKDROP_SIDE.min(side);
    let mut result = vec![0u8; (out * out * 4) as usize];
    for oy in 0..out {
        for ox in 0..out {
            let (sx0, sx1) = (
                x0 + ox * side / out,
                x0 + ((ox + 1) * side / out).max(ox * side / out + 1),
            );
            let (sy0, sy1) = (
                y0 + oy * side / out,
                y0 + ((oy + 1) * side / out).max(oy * side / out + 1),
            );
            let mut sum = [0u64; 4];
            let mut count = 0u64;
            for y in sy0..sy1.min(height) {
                for x in sx0..sx1.min(width) {
                    let i = ((y * width + x) * 4) as usize;
                    for c in 0..4 {
                        sum[c] += pixels[i + c] as u64;
                    }
                    count += 1;
                }
            }
            let o = ((oy * out + ox) * 4) as usize;
            for c in 0..4 {
                result[o + c] = (sum[c] / count.max(1)) as u8;
            }
        }
    }
    (out, out, result)
}

/// Repeated box blur, which approximates a gaussian on the thumbnail.
fn blur((width, height, mut pixels): (u32, u32, Vec<u8>)) -> (u32, u32, Vec<u8>) {
    let (w, h) = (width as i32, height as i32);
    let mut scratch = pixels.clone();
    for _ in 0..BLUR_PASSES {
        for (horizontal, src_is_pixels) in [(true, true), (false, false)] {
            let (src, dst) = if src_is_pixels {
                (&pixels, &mut scratch)
            } else {
                (&scratch, &mut pixels)
            };
            for y in 0..h {
                for x in 0..w {
                    let mut sum = [0u32; 4];
                    for d in -BLUR_RADIUS..=BLUR_RADIUS {
                        let (sx, sy) = if horizontal {
                            ((x + d).clamp(0, w - 1), y)
                        } else {
                            (x, (y + d).clamp(0, h - 1))
                        };
                        let i = ((sy * w + sx) * 4) as usize;
                        for c in 0..4 {
                            sum[c] += src[i + c] as u32;
                        }
                    }
                    let o = ((y * w + x) * 4) as usize;
                    for c in 0..4 {
                        dst[o + c] = (sum[c] / (2 * BLUR_RADIUS as u32 + 1)) as u8;
                    }
                }
            }
        }
    }
    (width, height, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backdrop_is_small_square_and_smooth() {
        let (w, h) = (300u32, 200u32);
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let v = if x < w / 2 { 255 } else { 0 };
                pixels[i..i + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        let (bw, bh, out) = blur(downscale(&pixels, w, h));
        assert_eq!((bw, bh), (BACKDROP_SIDE, BACKDROP_SIDE));
        assert!(out.chunks_exact(4).all(|p| p[3] == 255));
        let mid = ((bh / 2 * bw + bw / 2) * 4) as usize;
        assert!(out[mid] > 40 && out[mid] < 215, "edge is softened");
    }
}
