extern crate byteorder;

use std::env;
use std::io;
use std::io::{BufReader, BufWriter};
use std::f64::consts::PI;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("Usage: {} <samples per second> <mode>", args[0]);
        return;
    }

    let samples_per_second: f64 = args[1].parse::<f64>().expect("couldn't read samples per second parameter");
    let stdout = io::stdout();
    let mut sol = BufWriter::new(stdout.lock());
    let mut sg = SampleGenerator::new(samples_per_second, |s| sol.write_f32::<LittleEndian>(s));

    if args[2] == "stdin" {
        let stdin = io::stdin();
        for (f, d) in DualFloatTupleStdin::new(BufReader::new(stdin.lock())) {
            sg.consume(f, d)
        }
    } else {
        gen_freq_bits(true, |f, d| sg.consume(f, d));
    }
}

trait SstvMode {
    fn vis_code() -> u8;
    fn gen_image_tuples() -> Iterator<Item = (f32, f32)>;
}

fn gen_freq_bits<C>(vox_enabled: bool, mut consumer: C) where C: FnMut(f32, f32) {
    if vox_enabled {
        for freq in vec![1900.0, 1500.0, 1900.0, 1500.0, 2300.0, 1500.0, 2300.0, 1500.0] {
            consumer(freq, 100.0);
        }
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

struct SampleGenerator<C> {
    spms: f64,
    offset: f64,
    factor: f64,
    samples: f64,
    consumer: C,
}

impl<C: FnMut(f32) -> io::Result<()>> SampleGenerator<C> {
    fn new(samples_per_second: f64, consumer: C) -> SampleGenerator<C>
            where C: FnMut(f32) -> io::Result<()> {
        SampleGenerator {
            spms: samples_per_second / 1000.0,
            offset: 0.0,
            factor: 2.0 * PI / samples_per_second,
            samples: 0.0,
            consumer,
        }
    }

    fn consume(&mut self, freq: f32, msec: f32) {
        self.samples += self.spms * msec as f64;
        let tx = self.samples as i32;
        let freq_factor = freq as f64 * &self.factor;
        for sample in 0 .. tx {
            let output: f32 = (sample as f64 * freq_factor + self.offset).sin() as f32;
            (self.consumer)(output).expect("couldn't write float sample");
        }

        self.offset += (tx + 1) as f64 * freq_factor;
        self.samples -= tx as f64;
    }
}
