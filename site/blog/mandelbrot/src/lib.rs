use js_sys::{Uint8Array, Uint8ClampedArray};
use num::complex::ComplexFloat;
use serde::{Deserialize, Serialize};
use wasm_bindgen::{prelude::*, JsCast, JsValue};

/// Message Encoding and Decoding Format
pub trait Codec {
    /// Encode an input to JsValue
    fn encode<I>(input: I) -> JsValue
    where
        I: Serialize;

    /// Decode a message to a type
    fn decode<O>(input: JsValue) -> O
    where
        O: for<'de> Deserialize<'de>;
}

/// Default message encoding with [bincode].
#[derive(Debug)]
pub struct Postcard;

impl Codec for Postcard {
    fn encode<I>(input: I) -> JsValue
    where
        I: Serialize,
    {
        let buf = postcard::to_stdvec(&input).expect("failed to serialize a worker message");
        Uint8Array::from(buf.as_slice()).into()
    }

    fn decode<O>(input: JsValue) -> O
    where
        O: for<'de> Deserialize<'de>,
    {
        let data = Uint8Array::from(input).to_vec();
        postcard::from_bytes(&data).expect("failed to deserialize a worker message")
    }
}

// We need to be able to construct `ImageData` from an external typed array
// because it can't accept shared data
// (which is what's underlying the WASM linear memory).
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen]
    #[derive(Debug)]
    type ImageData;

    #[wasm_bindgen(constructor)]
    fn new(array: &Uint8ClampedArray, sw: u32) -> ImageData;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerParameters {
    pub plane_params: PlaneParameters,
    pub max_iterations: u32,
}

#[wasm_bindgen]
pub fn process(params: JsValue) -> JsValue {
    console_error_panic_hook::set_once();

    let params: WorkerParameters = Postcard::decode(params);

    let mut plane = Plane::new(params.plane_params);
    plane.update(params.max_iterations);

    Postcard::encode(plane)
}

pub type Real = f64;
pub type Complex = num::complex::Complex64;

fn divergence(c: Complex, max_iterations: u32) -> f64 {
    let mut z = Complex::new(0.0, 0.0);
    let mut iteration = 0;
    let mut smooth_iter = 0.0;

    while iteration < max_iterations && z.norm_sqr() <= 4.0 {
        z = z * z + c;
        iteration += 1;
    }

    if iteration < max_iterations {
        let log_zn = z.norm_sqr().ln() / 2.0;
        let nu = (log_zn / std::f64::consts::LN_2).ln() / std::f64::consts::LN_2;
        smooth_iter = iteration as f64 + 1.0 - nu;
    } else {
        smooth_iter = iteration as f64;
    }

    smooth_iter
}

fn color_from_palette(brightness: f64) -> [u8; 3] {
    let hue = 360.0 * brightness;
    hsv_to_rgb(hue % 360.0, 1.0, brightness.powf(0.3))
}

fn hsv_to_rgb(h: f64, s: f64, v: f64) -> [u8; 3] {
    let c = v * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - ((h_prime % 2.0) - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = match h_prime as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        5 | 6 => (c, 0.0, x),
        _ => (0.0, 0.0, 0.0),
    };

    [
        ((r1 + m) * 127.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
    ]
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PlaneParameters {
    width: usize,
    height: usize,
    position: (Real, Real),
    window: (Real, Real),
    y_offset: usize,
    total_height: usize,
}

impl PlaneParameters {
    pub fn new(width: usize, height: usize, position: (Real, Real), window: (Real, Real)) -> Self {
        Self {
            width,
            height,
            position,
            window,
            y_offset: 0,
            total_height: height,
        }
    }

    pub fn split(self, bands: usize) -> Vec<Self> {
        let PlaneParameters {
            width,
            height,
            position,
            window,
            ..
        } = self;

        let slice_height = height / bands;
        let mut sub_planes = Vec::with_capacity(bands);

        for i in 0..bands {
            let y_offset = i * slice_height;
            let current_height = if i == bands - 1 {
                height - y_offset
            } else {
                slice_height
            };

            let sub_params = PlaneParameters {
                width,
                height: current_height,
                position,
                window,
                y_offset,
                total_height: height,
            };
            sub_planes.push(sub_params);
        }

        sub_planes
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plane {
    buffer: Vec<u8>,
    params: PlaneParameters,
}

impl Plane {
    const CHANNELS: usize = 4;

    pub fn new(params: PlaneParameters) -> Self {
        let buffer = vec![0u8; params.width * params.height * Self::CHANNELS];
        Self { buffer, params }
    }

    #[inline]
    fn get_mut(&mut self, x: usize, y: usize) -> Option<&mut [u8; Self::CHANNELS]> {
        if x >= self.params.width || y >= self.params.height {
            return None;
        }
        let index = (x + y * self.params.width) * Self::CHANNELS;
        Some(
            (&mut self.buffer[index..index + Self::CHANNELS])
                .try_into()
                .unwrap(),
        )
    }

    #[inline]
    fn complex_at(&self, x: Real, y: Real) -> Complex {
        let width = self.params.width as f64;
        let height = self.params.total_height as f64;

        let real_ratio = x / width;
        let real_value = real_ratio * self.params.window.0 - self.params.window.0 / 2.0;

        let imaginary_ratio = (y + self.params.y_offset as f64) / height;
        let imaginary_value = imaginary_ratio * self.params.window.1 - self.params.window.1 / 2.0;

        Complex::new(
            real_value + self.params.position.0,
            imaginary_value + self.params.position.1,
        )
    }

    pub fn update(&mut self, max_iterations: u32) {
        let threshold = 0.05;
        let samples = 8;

        for y in 0..self.params.height {
            let mut prev_brightness = None::<f64>;
            for x in 0..self.params.width {
                let x_f64 = x as f64 + 0.5;
                let y_f64 = y as f64 + 0.5;
                let c = self.complex_at(x_f64, y_f64);
                let div = divergence(c, max_iterations);
                let brightness = div / max_iterations as f64;

                let need_supersampling = if let Some(prev) = prev_brightness {
                    (brightness - prev).abs() > threshold
                } else {
                    false
                };

                let final_brightness = if need_supersampling {
                    let mut brightness_accumulator = 0.0;
                    for sy in 0..samples {
                        for sx in 0..samples {
                            let sub_x = x as f64 + (sx as f64 + 0.5) / samples as f64;
                            let sub_y = y as f64 + (sy as f64 + 0.5) / samples as f64;
                            let c = self.complex_at(sub_x, sub_y);
                            let div = divergence(c, max_iterations);
                            brightness_accumulator += div / max_iterations as f64;
                        }
                    }
                    brightness_accumulator / (samples * samples) as f64
                } else {
                    brightness
                };

                let color = color_from_palette(final_brightness);
                let pixel = self.get_mut(x, y).unwrap();
                pixel[0] = color[0];
                pixel[1] = color[1];
                pixel[2] = color[2];
                pixel[3] = 255;

                prev_brightness = Some(brightness);
            }
        }
    }

    pub fn to_image_data(&self) -> web_sys::ImageData {
        let array = js_sys::Uint8ClampedArray::new_with_length(self.buffer.len() as u32);
        array.copy_from(&self.buffer);

        ImageData::new(&array, self.params.width as u32)
            .dyn_into()
            .unwrap()
    }

    pub fn recombine(planes: Vec<Plane>) -> Self {
        if planes.is_empty() {
            panic!("Planes must not be empty");
        }

        let mut params = planes[0].params;
        params.height = params.total_height;

        let height = planes[0].params.total_height;
        let width = planes[0].params.width;

        let mut final_buffer = Vec::with_capacity(width * height * Plane::CHANNELS);
        for buffer in planes.into_iter().map(|p| p.buffer) {
            final_buffer.extend(buffer);
        }

        Self {
            buffer: final_buffer,
            params,
        }
    }
}

fn recombine_buffers(buffers: Vec<Vec<u8>>, width: usize, height: usize) -> Vec<u8> {
    let mut final_buffer = Vec::with_capacity(width * height * Plane::CHANNELS);
    for buffer in buffers {
        final_buffer.extend(buffer);
    }
    final_buffer
}

fn render_fractal(
    full_params: PlaneParameters,
    max_iterations: u32,
    num_workers: usize,
) -> Vec<u8> {
    let sub_planes = full_params.split(num_workers);

    let mut buffers = Vec::new();
    for sub_params in sub_planes {
        let mut plane = Plane::new(sub_params);
        plane.update(max_iterations);
        buffers.push(plane.buffer);
    }

    let final_buffer = recombine_buffers(buffers, full_params.width, full_params.height);
    final_buffer
}
