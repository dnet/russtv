extern crate byteorder;

use std::env;
use std::io;
use std::io::{BufReader, BufWriter};
use std::f64::consts::PI;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <samples per second>", args[0]);
        return;
    }

    let samples_per_second: f64 = args[1].parse::<f64>().expect("couldn't read samples per second parameter");

    let spms = samples_per_second / 1000.0;
    let mut offset = 0.0;
    let factor = 2.0 * PI / samples_per_second;
    let mut samples = 0.0;
    let mut tx: i32;

    let stdin = io::stdin();
    let mut src = DualFloatTupleStdin::new(BufReader::new(stdin.lock()));
    let stdout = io::stdout();
    let mut sol = BufWriter::new(stdout.lock());

    for (freq, msec) in & mut src {
        samples += spms * msec as f64;
        tx = samples as i32;
        let freq_factor = freq as f64 * factor;
        for sample in 0 .. tx {
            let output: f32 = (sample as f64 * freq_factor + offset).sin() as f32;
            sol.write_f32::<LittleEndian>(output).expect("couldn't write float sample");
        }

        offset += (tx + 1) as f64 * freq_factor;
        samples -= tx as f64;
    }
}

struct DualFloatTupleStdin<'a> {
    sil: BufReader<io::StdinLock<'a>>,
}

impl<'a> DualFloatTupleStdin<'a> {
    fn new(sil: BufReader<io::StdinLock<'a>>) -> DualFloatTupleStdin<'a> {
        DualFloatTupleStdin { sil }
    }
}

impl<'a> Iterator for DualFloatTupleStdin<'a> {
    type Item = (f32, f32);

    fn next(&mut self) -> Option<(f32, f32)> {
        match self.sil.read_f32::<LittleEndian>() {
            Err(e) => match e.kind() {
                io::ErrorKind::UnexpectedEof => None,
                _ => panic!("Can't read frequency: {}", e),
            }
            Ok(freq) => Some((freq, self.sil.read_f32::<LittleEndian>().expect("couldn't read duration"))),
        }
    }
}
