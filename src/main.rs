use paulstretch_rust::stretch::paulstretch;
use paulstretch_rust::wav_helper;

use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    in_name: String,

    out_name: String,

    #[clap(short, default_value_t = 8.0)]
    stretch_factor: f32,

    #[clap(short, default_value_t = 0.25)]
    window_size_secs: f32,
}

fn main() {
    let args = Args::parse();
    let (header, samples) = wav_helper::load(&args.in_name).unwrap();

    println!(
        "loaded file (bit_depth: {}, sample_rate: {})",
        header.bits_per_sample, header.sampling_rate
    );

    let stretched = paulstretch(
        samples,
        header.sampling_rate,
        args.window_size_secs,
        args.stretch_factor,
    );

    println!("exporting...");
    wav_helper::export(&args.out_name, header, stretched).unwrap();
}
