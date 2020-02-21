use std::{env, fs};
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::time::Instant;

use opencv::{core, imgcodecs, imgproc};
use opencv::prelude::Vector;
use opencv::types::{VectorOfint, VectorOfMat, VectorOfuchar};

#[derive(Debug, Clone, Copy)]
pub struct ImageResize {
    pub width: i32,
    pub height: i32,
    pub vertical_border: i32,
    pub horizontal_border: i32,
}

const WHITE_COLOR: f64 = 255 as f64;

fn get_target_size(img_ref: core::Size, width: i32, height: i32) -> ImageResize {
    let radio: f32 = min(width as f32 / img_ref.width as f32, height as f32 / img_ref.height as f32);
    let mut new_width: i32 = (img_ref.width as f32 * radio) as i32;
    let mut new_height: i32 = (img_ref.height as f32 * radio) as i32;
    let mut v_border = 0;
    let mut h_border = 0;

    if new_height > new_width {
        let border: f32 = (new_height - new_width) as f32 / 2.0;
        h_border = border as i32;
        new_width += (border as i32 % 1) * 2;
    } else if new_width > new_height {
        let border: f32 = (new_width - new_height) as f32 / 2.0;
        v_border = { border as i32 };
        new_height += ((border as i32) % 1) * 2;
    } else {
        v_border = 0;
        h_border = 0;
    }

    ImageResize {
        width: new_width,
        height: new_height,
        vertical_border: v_border,
        horizontal_border: h_border,
    }
}

fn min(n1: f32, n2: f32) -> f32 {
    if n1 < n2 {
        n1
    } else if n2 < n1 {
        n2
    } else {
        n1
    }
}

fn read_file(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap()
}

fn read_image(buffer: &[u8]) -> Result<core::Mat, opencv::Error> {
    let src = core::Mat::from_slice(buffer)?;
    let dest = imgcodecs::imdecode(&src, imgcodecs::IMREAD_UNCHANGED)?;

    Ok(dest)
}

fn write_file_in_disk(buffer: &VectorOfuchar, path: PathBuf) -> () {
    let mut _file = File::create(path).expect("Error create file");
    _file.write(buffer.to_slice()).expect("no write");
    _file.flush().expect("no flush");
}

fn image_resize(image: &core::Mat, size: core::Size) -> Result<core::Mat, opencv::Error> {
    let mut result = core::Mat::default()?;
    imgproc::resize(
        image,
        &mut result,
        size,
        0f64,
        0f64,
        imgproc::INTER_AREA,
    ).expect("");
    Ok(result)
}

fn get_jpeg_buffer(image: &core::Mat) -> VectorOfuchar {
    let mut dest = VectorOfuchar::new();
    let mut quality = VectorOfint::with_capacity(2);
    quality.push(90);
    quality.push(imgcodecs::IMWRITE_JPEG_QUALITY);

    imgcodecs::imencode(
        ".jpg",
        &image,
        &mut dest,
        &quality,
    ).expect("");

    dest
}

pub fn expand(src: &core::Mat, resize: ImageResize) -> Result<core::Mat, opencv::Error> {
    let mut result = core::Mat::default()?;

    core::copy_make_border(src, &mut result, resize.vertical_border, resize.vertical_border,
                           resize.horizontal_border, resize.horizontal_border, core::BORDER_CONSTANT,
                           core::Scalar::all(WHITE_COLOR))
        .expect("not load buffer");
    Ok(result)
}

fn is_alpha_channel(src: &core::Mat) -> Result<bool, opencv::Error> {
    let mut split = VectorOfMat::new();
    core::split(&src, &mut split)?;
    Ok(split.len() == 4)
}

fn change_alpha_channels(src: &core::Mat) -> Result<core::Mat, opencv::Error> {
    let mut split = VectorOfMat::new();

    // split channels
    core::split(&src, &mut split)?;
    // set alpha
    let mut alpha = VectorOfMat::with_capacity(1);
    alpha.push(split.get(3)?);

    // remove alpha
    split.remove(3)?;

    let mut image = core::Mat::default()?;
    let mut alpha_image = core::Mat::default()?;

    // merge
    core::merge(&split, &mut image)?;
    core::merge(&alpha, &mut alpha_image)?;

    let mut bit_not = core::Mat::default()?;
    let mut bit_not_dest = core::Mat::default()?;
    let empty_mat = core::Mat::default()?;

    // invert colors
    core::bitwise_not(&alpha_image, &mut bit_not, &empty_mat)?;
    imgproc::cvt_color(&bit_not, &mut bit_not_dest, imgproc::COLOR_GRAY2RGB, 0)?;

    // bit and add
    let mut bit_and = core::Mat::default()?;
    let mut result = core::Mat::default()?;

    core::bitwise_and(&image, &image, &mut bit_and, &alpha_image)?;
    core::add(&bit_and, &bit_not_dest, &mut result, &empty_mat, 0)?;

    Ok(result)
}


fn main() {
    let args: Vec<String> = env::args().collect();
    // Init params
    let path: &Path = Path::new(args[1].as_str());
    let square: i32 = args[2].parse().unwrap();
    let buffer = read_file(path);
    let start_total = Instant::now();
    let mut start = Instant::now();
    
    // Load image
    let mut image = read_image(&buffer[..]).unwrap();
    let is_alpha = is_alpha_channel(&image).unwrap();

    println!("time to read image from buffer alpha={:?} w={:?} h={:?} time={:?}", is_alpha, image.cols(), image.rows(), start.elapsed());

    // Resize
    start = Instant::now();
    let positions = get_target_size(
        core::Size {
            width: image.cols().unwrap(),
            height: image.rows().unwrap(),
        },
        square,
        square,
    );
    image = image_resize(&image, core::Size { width: positions.width, height: positions.height }).unwrap();
    println!("time to resize w={:?} h={:?} time={:?}", image.cols(), image.rows(), start.elapsed());

    // Extend
    if image.cols().unwrap() != image.rows().unwrap() {
        start = Instant::now();
        image = expand(&image, positions).unwrap();
        println!("time to extend time  w={:?} h={:?} time={:?}", image.cols(), image.rows(), start.elapsed());
    }

    if is_alpha {
        start = Instant::now();
        image = change_alpha_channels(&image).unwrap();
        println!("time to remove alpha time  w={:?} h={:?} time={:?}", image.cols(), image.rows(), start.elapsed());
    }
    
    // Read Buffer
    start = Instant::now();
    let buffer = get_jpeg_buffer(&image);
    println!("time to read buffer size={:?} time={:?}", buffer.len(), start.elapsed());
    println!("total time {:?}", start_total.elapsed());
    
    // Only write
    start = Instant::now();
    write_file_in_disk(&buffer, path.parent().unwrap().join(format!("out_{}_opencv.jpeg", path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap())));
    println!("time to write disk size={:?} time={:?}", buffer.len(), start.elapsed());
}
