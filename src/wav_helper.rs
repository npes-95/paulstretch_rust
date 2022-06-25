use wav::{bit_depth::BitDepth, header::Header};

use std::fs::File;
use std::io;
use std::path::Path;

pub fn load(path: &str) -> io::Result<(Header, Vec<f32>)> {
    let mut f = File::open(Path::new(path))?;
    let (header, data) = wav::read(&mut f)?;
    let samples = to_bounded_f32(data);
    Ok((header, samples))
}

pub fn export(path: &str, header: Header, samples: Vec<f32>) -> io::Result<()> {
    let data = from_bounded_f32(header, samples);
    let mut f = File::create(Path::new(path))?;
    wav::write(header, &data, &mut f)
}

fn to_bounded_f32(data: BitDepth) -> Vec<f32> {
    match data {
        BitDepth::Eight(s) => s
            .iter()
            .map(|s| *s as f32 * (2_f32 / u8::MAX as f32) - 1_f32)
            .collect(),
        BitDepth::Sixteen(s) => s
            .iter()
            .map(|s| *s.clamp(&-i16::MAX, &i16::MAX) as f32 / i16::MAX as f32)
            .collect(),
        BitDepth::TwentyFour(s) => s.iter().map(|s| *s as f32 / 0x7FFFFF as f32).collect(),
        BitDepth::ThirtyTwoFloat(s) => s,
        _ => panic!(),
    }
}

fn from_bounded_f32(header: Header, samples: Vec<f32>) -> BitDepth {
    match header.bits_per_sample {
        8 => BitDepth::Eight(
            samples
                .iter()
                .map(|s| (((s + 1_f32) / 2_f32) * u8::MAX as f32) as u8)
                .collect(),
        ),
        16 => BitDepth::Sixteen(
            samples
                .iter()
                .map(|s| (s * i16::MAX as f32) as i16)
                .collect(),
        ),
        24 => BitDepth::TwentyFour(
            samples
                .iter()
                .map(|s| (s * 0x7FFFFF as f32) as i32)
                .collect(),
        ),
        32 => BitDepth::ThirtyTwoFloat(samples),
        _ => panic!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: reimplement get_min and get_max
    // TODO: fill vector with random data and check bounds
    // TODO: refactor and make generic

    #[test]
    fn u8_to_f32() {
        let data_min = BitDepth::Eight(vec![u8::MIN; 1]);
        let data_max = BitDepth::Eight(vec![u8::MAX; 1]);
        let samples_min = to_bounded_f32(data_min);
        let samples_max = to_bounded_f32(data_max);
        assert!(samples_min[0] >= -1_f32);
        assert!(samples_max[0] <= 1_f32);
    }

    #[test]
    fn i16_to_f32() {
        let data_min = BitDepth::Sixteen(vec![i16::MIN; 1]);
        let data_max = BitDepth::Sixteen(vec![i16::MAX; 1]);
        let samples_min = to_bounded_f32(data_min);
        let samples_max = to_bounded_f32(data_max);
        assert!(samples_min[0] >= -1_f32);
        assert!(samples_max[0] <= 1_f32);
    }

    #[test]
    fn i24_to_f32() {
        let data_min = BitDepth::TwentyFour(vec![-0x7FFFFF_i32; 1]);
        let data_max = BitDepth::TwentyFour(vec![0x7FFFFF_i32; 1]);
        let samples_min = to_bounded_f32(data_min);
        let samples_max = to_bounded_f32(data_max);
        assert!(samples_min[0] >= -1_f32);
        assert!(samples_max[0] <= 1_f32);
    }

    #[test]
    fn f32_to_f32() {
        let data_min = BitDepth::ThirtyTwoFloat(vec![-1_f32; 1]);
        let data_max = BitDepth::ThirtyTwoFloat(vec![1_f32; 1]);
        let samples_min = to_bounded_f32(data_min);
        let samples_max = to_bounded_f32(data_max);
        assert!(samples_min[0] >= -1_f32);
        assert!(samples_max[0] <= 1_f32);
    }
}
