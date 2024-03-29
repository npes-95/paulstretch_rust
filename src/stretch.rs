use rand::distributions::{Distribution, Uniform};
use realfft::RealFftPlanner;
use rustfft::num_complex::Complex;

use std::f32::consts::PI;
use std::usize;

fn compute_end_size(sample_rate: u32) -> usize {
    let end_size = sample_rate / 20;
    if end_size < 16 {
        16
    } else {
        end_size as usize
    }
}

fn compute_window_size(window_size_secs: f32, sample_rate: u32) -> usize {
    let window_size = (window_size_secs * sample_rate as f32) as usize;
    if window_size < 16 {
        16
    } else {
        window_size - (window_size % 2)
    }
}

fn compute_linspace(x0: f32, xn: f32, n: usize) -> Vec<f32> {
    let dx = (xn - x0) / ((n - 1) as f32);
    (0..n).map(|i| x0 + i as f32 * dx).collect()
}

fn compute_window_func(window_size: usize) -> Vec<f32> {
    compute_linspace(-1_f32, 1_f32, window_size)
        .iter()
        .map(|i| (1_f32 - i.powf(2_f32)).powf(1.25))
        .collect()
}

fn overlap_add(current: &Vec<f32>, prev: &Vec<f32>, added: &mut Vec<f32>) {
    assert_eq!(current.len(), prev.len());
    assert_eq!(added.len(), current.len() / 2);
    let current_front = &current[..current.len() / 2];
    let prev_back = &prev[prev.len() / 2..];

    assert_eq!(current_front.len(), prev_back.len());
    for (c, (p, a)) in current_front
        .iter()
        .zip(prev_back.iter().zip(added.iter_mut()))
    {
        *a = *c + *p;
    }
}

pub fn paulstretch_multichannel(
    mut samples: Vec<Vec<f32>>,
    sample_rate: u32,
    window_size_secs: f32,
    stretch_factor: f32,
    indicate_progress: &impl Fn(u32, u32),
) -> Vec<Vec<f32>> {
    let mut out = Vec::with_capacity(samples.len());
    for channel in samples.drain(..) {
        out.push(paulstretch(
            channel,
            sample_rate,
            window_size_secs,
            stretch_factor,
            indicate_progress,
        ))
    }
    out
}

pub fn paulstretch(
    mut samples: Vec<f32>,
    sample_rate: u32,
    window_size_secs: f32,
    stretch_factor: f32,
    indicate_progress: &impl Fn(u32, u32),
) -> Vec<f32> {
    // correct end size of data
    let end_size = compute_end_size(sample_rate);
    let linspace = compute_linspace(0_f32, 1_f32, end_size);
    assert!(end_size >= 16);
    assert_eq!(linspace.len(), end_size);
    for (s, l) in samples.iter_mut().rev().zip(linspace.iter()) {
        *s *= *l;
    }
    assert_eq!(samples.last().unwrap(), &0_f32);

    // compute window size and allocate buffers
    let window_size = compute_window_size(window_size_secs, sample_rate);
    let half_window_size = window_size / 2;
    assert!(window_size >= 16);

    let mut cur_buffer = vec![0_f32; window_size];
    let mut prev_buffer = vec![0_f32; window_size];
    let mut out_buffer = vec![0_f32; half_window_size];

    let window = compute_window_func(window_size);

    // init loop control
    let mut start = 0_f32;
    let step = half_window_size as f32 / stretch_factor;

    // allocate output buffer
    let mut output = Vec::with_capacity((samples.len() as f32 / step) as usize * half_window_size);

    // init FFT
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(window_size);
    let ifft = planner.plan_fft_inverse(window_size);
    let mut spectrum = fft.make_output_vec();
    let mut scratch_forward = fft.make_scratch_vec();
    let mut scratch_inverse = ifft.make_scratch_vec();
    let fft_scale = 1_f32 / window_size as f32;
    let spectrum_is_even = spectrum.len() % 2 == 0;

    // init rand
    let uniform = Uniform::new(0_f32, 2_f32 * PI);
    let mut rng = rand::thread_rng();

    // progress counter
    let mut iters = 0;
    let max_iters = (samples.len() as f32 / step) as u32;

    loop {
        indicate_progress(iters, max_iters);

        // grab window_size samples and pad with zeros if there aren't enough left
        let remaining = samples.len() - start as usize;
        if remaining > window_size {
            cur_buffer.copy_from_slice(&samples[(start as usize)..(start as usize + window_size)]);
        } else {
            cur_buffer[remaining..].fill(0_f32);
            cur_buffer[..remaining].copy_from_slice(&samples[(start as usize)..]);
            assert_eq!(cur_buffer.last(), Some(&0_f32));
        }

        // apply window function
        for (s, w) in cur_buffer.iter_mut().zip(window.iter()) {
            *s *= *w;
        }

        // get the amplitudes of the frequency components
        fft.process_with_scratch(&mut cur_buffer, &mut spectrum, &mut scratch_forward)
            .unwrap();

        //randomize the phases by multiplication with a random complex number with modulus=1
        spectrum.iter_mut().for_each(|f| {
            let rand_complex = Complex::new(0_f32, uniform.sample(&mut rng));
            *f = Complex::new(f.norm(), f.norm()) * rand_complex.exp();
        });

        // realfft expects data in the form:
        // [(X0r, 0), (X1r, X1i), (X2r, X2i), (X3r, 0)] for even len
        // [(X0r, 0), (X1r, X1i), (X2r, X2i), (X3r, X3i)] for odd len
        spectrum[0].im = 0_f32;
        if spectrum_is_even {
            spectrum[half_window_size].im = 0_f32;
        }

        ifft.process_with_scratch(&mut spectrum, &mut cur_buffer, &mut scratch_inverse)
            .unwrap();

        // normalize fft output by scaling 1/len
        cur_buffer.iter_mut().for_each(|s| *s *= fft_scale);

        // apply window function again
        for (s, w) in cur_buffer.iter_mut().zip(window.iter()) {
            *s *= *w;
        }

        overlap_add(&cur_buffer, &prev_buffer, &mut out_buffer);
        prev_buffer.copy_from_slice(&cur_buffer.as_slice());

        out_buffer
            .iter_mut()
            .for_each(|s| *s = s.clamp(-1_f32, 1_f32));

        start += step;

        if start as usize >= samples.len() {
            return output;
        }

        output.extend_from_slice(&out_buffer);

        iters += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_size() {
        let sample_rates = vec![8000, 16000, 44100, 48000, 96000, 128000];

        for sr in sample_rates {
            let end_size = compute_end_size(sr);
            assert!(end_size >= 16);
        }
    }

    #[test]
    fn window_size() {
        let sizes = vec![0.0, 0.05, 0.1, 0.15, 0.2, 0.25, 0.3, 0.35, 0.4];
        let sample_rates = vec![8000, 16000, 44100, 48000, 96000, 128000];

        for sr in sample_rates {
            for s in sizes.iter() {
                let window_size = compute_window_size(*s, sr);
                assert!(window_size >= 16);
                assert_eq!(window_size % 2, 0);
            }
        }
    }

    #[test]
    fn linspace() {
        let linspace = compute_linspace(0_f32, 1_f32, 3);
        assert_eq!(linspace.len(), 3);
        assert_eq!(linspace[0], 0_f32);
        assert_eq!(linspace[1], 0.5);
        assert_eq!(linspace[2], 1_f32);
    }

    #[test]
    fn overlap() {
        let v1 = vec![0_f32, 1_f32, 2_f32, 3_f32];
        let v2 = vec![4_f32, 5_f32, 6_f32, 7_f32];
        let mut added = vec![0_f32, 0_f32];
        overlap_add(&v1, &v2, &mut added);
        assert_eq!(added, vec![6_f32, 8_f32]);
    }
}
