use std::{env, fs};
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::time::{Instant, Duration};
use opencv::{core, imgcodecs, imgproc};
use opencv::prelude::Vector;
use opencv::types::{VectorOfint, VectorOfMat, VectorOfuchar};
use regex::Regex;

fn min(n1: f32, n2: f32) -> f32 {
    if n1 == n2 {
        return n1;
    }
    if n1 < n2 {
        return n1;
    }
    return n2;
}

fn read_file(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap()
}

fn read_image(buffer: &[u8]) -> Result<core::Mat, opencv::Error> {
    let src = core::Mat::from_slice(buffer)?;
    let target = imgcodecs::imdecode(&src, imgcodecs::IMREAD_UNCHANGED)?;

    Ok(target)
}

fn write_file_in_disk(buffer: &VectorOfuchar, path: &Path, prefix: &str) -> () {
    let re = Regex::new(r"^(.*?)\..*$").unwrap();
    let result = re.replace_all(path.file_name().unwrap().to_str().unwrap(), "$1");

    let mut _file = File::create(path.parent().unwrap().join(format!("{}_rust_{}.jpg", result.as_ref(), prefix).as_str())).expect("Error create file");
    _file.write(buffer.to_slice()).expect("no write");
    _file.flush().expect("no flush");
}

fn image_resize(image: &core::Mat, width: i32, height: i32) -> Result<core::Mat, opencv::Error> {
    let mut result = core::Mat::default()?;
    imgproc::resize(
        image,
        &mut result,
        core::Size {
            width,
            height,
        },
        0f64,
        0f64,
        imgproc::INTER_AREA,
    ).expect("");
    Ok(result)
}

fn get_jpeg_buffer(image: &core::Mat, quality: &VectorOfint) -> VectorOfuchar {
    let mut dest: VectorOfuchar = VectorOfuchar::new();
    imgcodecs::imencode(
        ".jpg",
        &image,
        &mut dest,
        &quality,
    ).expect("");

    dest
}

fn fill(src: &core::Mat, vertical_border: i32, horizontal_border: i32) -> Result<core::Mat, opencv::Error> {
    let mut result = core::Mat::default()?;
    core::copy_make_border(src,
                           &mut result,
                           vertical_border,
                           vertical_border,
                           horizontal_border,
                           horizontal_border,
                           core::BORDER_CONSTANT,
                           core::Scalar::all(255.0))?;
    Ok(result)
}

fn change_alpha_channels(split: &mut VectorOfMat) -> Result<core::Mat, opencv::Error> {
    // set alpha
    let mut alpha = VectorOfMat::with_capacity(1);
    alpha.push(split.get(3)?);
    // remove alpha
    split.remove(3)?;

    let mut colors = VectorOfMat::with_capacity(3);
    colors.push(split.get(0)?);
    colors.push(split.get(1)?);
    colors.push(split.get(2)?);

    let mut image = core::Mat::default()?;
    let mut alpha_image = core::Mat::default()?;
    // merge
    core::merge(&colors, &mut image)?;
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

    alpha.clear();
    colors.clear();

    image.release()?;
    alpha_image.release()?;
    bit_not.release()?;
    bit_and.release()?;
    bit_not_dest.release()?;

    Ok(result)
}

fn verify_params(params: &Vec<String>) -> bool {
    if params.len() < 3 {
        println!("Illegal arguments was expected image path and size\n\
        Example: /home/user/Pictures/image.jpg 700");
        return false;
    }
    return true;
}

fn get_sizes(params: &Vec<String>, index: usize) -> Vec<f32> {
    let mut sizes: Vec<f32> = params.into_iter()
        .skip(index)
        .map(|s| s.parse::<f32>().unwrap())
        .rev()
        .collect();
    sizes.sort_by(|a, b| b.partial_cmp(a).unwrap());
    sizes
}

fn get_channel(src: &core::Mat) -> Result<VectorOfMat, opencv::Error> {
    let mut split = VectorOfMat::new();
    core::split(&src, &mut split)?;
    Ok(split)
}

fn get_target_new_size(square: &f32, src: &core::Mat) -> (i32, i32, i32, i32) {
    let (width, height) = (src.cols().unwrap() as f32, src.rows().unwrap() as f32);
    let radio: f32 = min(square / width, square / height);
    let (mut new_width, mut new_height) = ((width * radio), (height * radio));
    let (mut v_border, mut h_border) = (0, 0);
    let border;

    if new_height == new_width {
        return (new_width as i32, new_height as i32, v_border, h_border);
    }

    if new_height > new_width {
        border = (new_height - new_width) / 2.0;
        h_border = border as i32;
        new_width += (border % 1.0) * 2.0;
    } else {
        border = (new_width - new_height) / 2.0;
        v_border = border as i32;
        new_height += (border % 1.0) * 2.0;
    }

    (new_width as i32, new_height as i32, v_border, h_border)
}

fn get_default_quality() -> VectorOfint {
    let mut quality: VectorOfint = VectorOfint::with_capacity(2);
    quality.push(90);
    quality.push(imgcodecs::IMWRITE_JPEG_QUALITY);
    quality
}

fn main() {
    let args: Vec<String> = env::args().collect();
    // Verify params number
    if !verify_params(&args) {
        return;
    }
    // Load Path Main Image
    let path: &Path = Path::new(args[1].as_str());
    if !path.exists() || !path.is_file() {
        println!("{:?} not exist ", path);
        return;
    }
    // Load Sizes to resize
    let sizes: Vec<f32> = get_sizes(&args, 2);
    // Read buffer of Image
    let buffer: Vec<u8> = read_file(path);
    // Default quality
    let quality: VectorOfint = get_default_quality();

    let mut start = Instant::now();
    // Load Image Matrix
    let mut image: core::Mat = read_image(&buffer[..]).unwrap();
    let mut all_process: u64 = start.elapsed().as_nanos() as u64;
    println!("Time to Load main image time={:?}", start.elapsed());
    // Load channels Image
    let mut i: i32 = 0;
    for square in &sizes {
        start = Instant::now();
        let positions = get_target_new_size(square, &image);
        image = image_resize(&image, positions.0, positions.1).unwrap();

        if i == 0 {
            let mut channels = get_channel(&image).unwrap();
            if channels.len() == 4 {
                image = change_alpha_channels(&mut channels).unwrap();
            }
        }

        if positions.0 != positions.1 {
            image = fill(&image, positions.2, positions.3).unwrap();
        }

        let buffer: VectorOfuchar = get_jpeg_buffer(&image, &quality);
        let duration = start.elapsed();
        all_process += duration.as_nanos() as u64;
        println!("Time to create image from buffer w={:?} h={:?} buffer={:?} Bytes time={:?}",
                 image.cols(),
                 image.rows(),
                 buffer.len(),
                 duration);
        // only out not sum in time
        write_file_in_disk(&buffer, path, i.to_string().as_str());
        i += 1;
    }

    image.release().expect("can't release image");
    println!("All process {:?} images in time={:?} check files in directory {:?}",
             sizes.len(),
             Duration::from_nanos(all_process),
             path.parent());
}
