extern crate byteorder;
extern crate image;

use std::env;
use std::io;
use std::io::{BufReader, BufWriter};
use std::f64::consts::PI;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use image::DynamicImage;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("Usage: {} <samples per second> <source>", args[0]);
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
        let pic = image::open(&args[2]).unwrap();
        let img = Box::new(GrayscaleSstv::new(SstvModes::Robot24BW, pic));
        gen_freq_bits(true, |f, d| sg.consume(f, d), img);
    }
}

trait SstvMode<C> {
    fn vis_code(&self) -> u8;
    fn gen_image_tuples(&self, consumer: C) -> ();
}

enum SstvModes {
    Robot8BW, Robot24BW,
}

struct GrayscaleSstv {
    vis_code: u8,
    width: u32,
    height: u32,
    scan: u8,
    pic: DynamicImage,
}

impl<C: FnMut(f32, f32)> SstvMode<C> for GrayscaleSstv {
    fn vis_code(&self) -> u8 {
        self.vis_code
    }

    fn gen_image_tuples(&self, mut consumer: C) {
        let img = self.pic.to_luma();
        if img.width() < self.width {
            panic!("Image width is smaller than required by selected mode");
        }
        if img.height() < self.height {
            panic!("Image height is smaller than required by selected mode");
        }
        let msec_pixel = ((self.scan as f64) / (self.width as f64)) as f32;
        for line in 0..self.height {
            consumer(FREQ_SYNC, 7.0);
            for col in 0..self.width {
                let pixel = img.get_pixel(col, line);
                let freq_pixel = byte_to_freq(pixel.data[0]);
                consumer(freq_pixel, msec_pixel);
            }
        }
    }
}

fn byte_to_freq(value: u8) -> f32 {
    FREQ_BLACK + FREQ_RANGE * (value as f32) / 255.0
}

impl GrayscaleSstv {
    fn new(mode: SstvModes, pic: DynamicImage) -> GrayscaleSstv {
        match mode {
            SstvModes::Robot8BW => GrayscaleSstv { vis_code: 0x02, width: 160,
                height: 120, scan: 60, pic },
            SstvModes::Robot24BW => GrayscaleSstv { vis_code: 0x0A, width: 320,
                height: 240, scan: 93, pic },
            _ => panic!("invalid mode"),
        }
    }
}

const FREQ_VIS_BIT1: f32 = 1100.0;
const FREQ_SYNC: f32 = 1200.0;
const FREQ_VIS_BIT0: f32 = 1300.0;
const FREQ_BLACK: f32 = 1500.0;
const FREQ_VIS_START: f32 = 1900.0;
const FREQ_WHITE: f32 = 2300.0;
const FREQ_RANGE: f32 = FREQ_WHITE - FREQ_BLACK;
const FREQ_FSKID_BIT1: f32 = 1900.0;
const FREQ_FSKID_BIT0: f32 = 2100.0;

const MSEC_VIS_START: f32 = 300.0;
const MSEC_VIS_SYNC: f32 = 10.0;
const MSEC_VIS_BIT: f32 = 30.0;
const MSEC_FSKID_BIT: f32 = 22.0;

fn gen_freq_bits<CNS>(vox_enabled: bool, mut consumer: CNS, mode: Box<SstvMode<CNS>>) where CNS: FnMut(f32, f32) {
    if vox_enabled {
        for freq in vec![1900.0, 1500.0, 1900.0, 1500.0, 2300.0, 1500.0, 2300.0, 1500.0] {
            consumer(freq, 100.0);
        }
    }
    consumer(FREQ_VIS_START, MSEC_VIS_START);
    consumer(FREQ_SYNC, MSEC_VIS_SYNC);
    consumer(FREQ_VIS_START, MSEC_VIS_START);
    consumer(FREQ_SYNC, MSEC_VIS_BIT);
    let mut vis = mode.vis_code();
    let mut num_ones = 0;
    for _ in 0..7 {
        let bit = vis & 1;
        vis >>= 1;
        num_ones += bit;
        let bit_freq = match bit { 1 => FREQ_VIS_BIT1, 0 => FREQ_VIS_BIT0, _ => panic!("bit not 1/0") };
        consumer(bit_freq, MSEC_VIS_BIT);
    }
    let parity_freq = match num_ones % 2 { 1 => FREQ_VIS_BIT1, 0 => FREQ_VIS_BIT0, _ => panic!("%2 not 1/0") };
    consumer(parity_freq, MSEC_VIS_BIT);
    consumer(FREQ_SYNC, MSEC_VIS_BIT);
    mode.gen_image_tuples(consumer);
    // TODO fskid
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
        let freq_factor = freq as f64 * self.factor;
        for sample in 0 .. tx {
            let output: f32 = (sample as f64 * freq_factor + self.offset).sin() as f32;
            (self.consumer)(output).expect("couldn't write float sample");
        }

        self.offset += (tx + 1) as f64 * freq_factor;
        self.samples -= tx as f64;
    }
}
