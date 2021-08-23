use image::math::utils::clamp;
use image::{ImageBuffer, Luma};

pub struct Dithering {
    normalization: u32,
    kernel: [[u32; 5]; 5],
}

pub const JARVIS_JUDICE_NINKE: Dithering = Dithering {
    normalization: 48,
    kernel: [
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 0, 7, 5],
        [3, 5, 7, 5, 3],
        [1, 3, 5, 3, 1],
    ],
};

pub const FLOYD_STEINBERG: Dithering = Dithering {
    normalization: 16,
    kernel: [
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 0, 7, 0],
        [0, 3, 5, 1, 0],
        [0, 0, 0, 0, 0],
    ],
};

pub const ATKINSON: Dithering = Dithering {
    normalization: 8,
    kernel: [
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 0, 1, 1],
        [0, 1, 1, 1, 0],
        [0, 0, 1, 0, 0],
    ],
};

pub const NO_DITHER: Dithering = Dithering {
    normalization: 1,
    kernel: [
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 1, 0, 0],
        [0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0],
    ],
};

fn is_inside_image(image: &ImageBuffer<Luma<u8>, Vec<u8>>, x: i64, y: i64) -> bool {
    !(x < 0 || y < 0 || x > image.width() as i64 - 1 || y > image.height() as i64 - 1)
}

fn kernel_by_delta(kernel: &[[u32; 5]; 5], dx: i64, dy: i64) -> u32 {
    let size = 5;
    let vx = (dx + (size - 1) / 2) as usize;
    let vy = (dy + (size - 1) / 2) as usize;

    // println!(
    //     "size: {}, dx: {}, dy: {}, vx: {}, vy: {}",
    //     size, dx, dy, vx, vy
    // );
    kernel[vy][vx]
}

pub fn apply_error_diffusion(
    mut image: ImageBuffer<Luma<u8>, Vec<u8>>,
    dither: Dithering,
) -> ImageBuffer<Luma<u8>, Vec<u8>> {
    let rows = 5;
    let cols = rows;
    for y in 0..image.height() {
        for x in 0..image.width() {
            let original = image.get_pixel(x, y).0[0];
            // Only look at the highest 3-bits
            let quantized_pixel = original & 0xe0;
            let quantization_error = original - quantized_pixel;

            let p = image.get_pixel_mut(x, y);
            p.0[0] = quantized_pixel;

            for dy in -2..=2 {
                for dx in -2..=2 {
                    // println!("dx: {}, dy: {}", dx, dy);
                    let kernel_value = kernel_by_delta(&dither.kernel, dx, dy);
                    // println!("{}", kernel_value);
                    let kx = i64::from(x) + dx;
                    let ky = i64::from(y) + dy;
                    if kernel_value != 0 && is_inside_image(&image, kx, ky) {
                        let p = image.get_pixel_mut(kx as u32, ky as u32);
                        let original = p.0[0];
                        let correction = ((quantization_error as f64 * kernel_value as f64)
                            / dither.normalization as f64)
                            .floor();

                        p.0[0] = clamp(original as f64 + correction, 0.0, 255.0) as u8;

                        // println!(
                        //     "before: {}, after: {}, correction: {}",
                        //     original, p.0[0], correction
                        // );
                    }
                }
            }
        }
    }

    image
}
