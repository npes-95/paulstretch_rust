use paulstretch_rust::stretch::paulstretch_multichannel;
use paulstretch_rust::wav_helper;

use clap::Parser;

use std::io::Write;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    in_file: String,

    #[clap(short)]
    out_file: Option<String>,

    #[clap(short, default_value_t = 8.0)]
    stretch_factor: f32,

    #[clap(short, default_value_t = 0.25)]
    window_size_secs: f32,
}

fn print_progress(current: u32, total: u32) {
    let ratio = current as f32 / total as f32;
    let percent = 100_f32 * ratio;
    let width = 30;
    let num_blocks = (width as f32 * ratio) as u32;

    print!("\r");
    print!("[");
    for _ in 0..num_blocks {
        print!("=");
    }
    for _ in 0..width - num_blocks {
        print!(" ");
    }
    print!("]");
    print!(" {}%", percent as u32);

    if num_blocks == width {
        print!("\n");
    }

    std::io::stdout().flush().unwrap();
}

fn main() {
    let args = Args::parse();

    let out_file = if let Some(out_file) = &args.out_file.as_deref() {
        out_file.to_string()
    } else {
        args.in_file.replacen(".wav", "-stretched.wav", 1)
    };

    let wave = wav_helper::load(&args.in_file).unwrap();

    println!(
        "loaded {} (channels: {}, bit_depth: {}, sample_rate: {})",
        &args.in_file, wave.header.channels, wave.header.bit_depth, wave.header.sample_rate
    );

    println!(
        "processing (stretch factor: {}, window size: {}s)",
        args.stretch_factor, args.window_size_secs
    );

    let stretched = paulstretch_multichannel(
        wave.data,
        wave.header.sample_rate,
        args.window_size_secs,
        args.stretch_factor,
        &print_progress,
    );

    println!("done!");

    println!("exporting {}", &out_file);

    wav_helper::export(
        &out_file,
        wav_helper::Wave {
            header: wave.header,
            data: stretched,
        },
    )
    .unwrap();
}
