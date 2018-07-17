extern crate byteorder;

fn main() {
    use std::env;
    use std::io;
    use std::f64::consts::PI;
    use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <samples per second>", args[0]);
        return;
    }

    let samples_per_second: f64 = args[1].parse::<f64>().unwrap();

    let spms = samples_per_second / 1000.0;
    let mut offset = 0.0;
    let factor = 2.0 * PI / samples_per_second;
    let mut samples = 0.0;
    let mut tx: i32;

    let stdin = io::stdin();
    let mut sil = stdin.lock();
    let stdout = io::stdout();
    let mut sol = stdout.lock();

    loop {
        match sil.read_f32::<LittleEndian>() {
            Ok(freq) => {
                let msec = sil.read_f32::<LittleEndian>().unwrap();

                samples += spms * msec as f64;
                tx = samples as i32;
                let freq_factor = freq as f64 * factor;
                for sample in 0 .. tx {
                    let output: f32 = (sample as f64 * freq_factor + offset).sin() as f32;
                    sol.write_f32::<LittleEndian>(output).unwrap();
                }

                offset += (tx + 1) as f64 * freq_factor;
                samples -= tx as f64;
            }
            Err(_e) => { return; }
        }
    }
}
