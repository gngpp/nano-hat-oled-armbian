use std::fs::OpenOptions;
use std::io::Write;
use std::thread;
use std::time::{Duration, SystemTime};

use embedded_graphics::image;

const I2C_BUS_PATH: &str = "/dev/i2c-0";
const I2C_DEVICE_ADDR: u16 = 0x3C;
const DISPLAY_WIDTH: usize = 128;
const DISPLAY_HEIGHT: usize = 64;
const CMD_INDEX_KEY1: u8 = 1;
const CMD_INDEX_KEY2: u8 = 2;
const CMD_INDEX_KEY3: u8 = 3;
const SHUTDOWN_CMD_INDEX: u8 = 99;

fn write_i2c_data(i2c_bus: &mut impl i2cdev::core::I2CDevice, data: &[u8]) -> std::io::Result<()> {
    i2c_bus.write(data);
    Ok(())
}

fn write_i2c_image_data(
    i2c_bus: &mut impl i2cdev::core::I2CDevice,
    image_data: &[u8],
) -> std::io::Result<()> {
    const BLOCK_SIZE: usize = 32;
    let mut block_data = Vec::new();
    let byte = 0;
    for (i, &pixel) in image_data.iter().enumerate() {
        let byte = (byte << 1) | pixel;
        if i % DISPLAY_WIDTH == DISPLAY_WIDTH - 1 {
            block_data.push(byte);
            if block_data.len() == BLOCK_SIZE {
                write_i2c_data(i2c_bus, &[0x40].iter().chain(block_data.iter()).copied().collect::<Vec<_>>())?;
                block_data.clear();
            }
        }
    }

    Ok(())
}

fn main() -> std::io::Result<()> {
    let mut i2c_bus = i2cdev::linux::LinuxI2CDevice::new(I2C_BUS_PATH, I2C_DEVICE_ADDR)?;
    let mut image_data = vec![0u8; DISPLAY_WIDTH * DISPLAY_HEIGHT / 8];

    let mut cmd_index = CMD_INDEX_KEY2;
    let mut display_refresh_time = SystemTime::now();

    // Initialize GPIO
    OpenOptions::new().write(true).open("/sys/class/gpio/export")?.write_all(b"0\n")?;
    OpenOptions::new().write(true).open("/sys/class/gpio/export")?.write_all(b"2\n")?;
    OpenOptions::new().write(true).open("/sys/class/gpio/export")?.write_all(b"3\n")?;
    OpenOptions::new().write(true).open("/sys/class/gpio/gpio0/direction")?.write_all(b"in\n")?;
    OpenOptions::new().write(true).open("/sys/class/gpio/gpio2/direction")?.write_all(b"in\n")?;
    OpenOptions::new().write(true).open("/sys/class/gpio/gpio3/direction")?.write_all(b"in\n")?;

    // Configure OLED display
    write_i2c_data(
        &mut i2c_bus,
        &[
            0x00, 0xAE, // set display off
            0x00, 0x00, // set lower column address
            0x00, 0x10, // set higher column address
            0x00, 0x40, // set display start line
            0x00, 0xB0,
            0x00, 0x81, // set page address
            0x00, 0xCF, // set screen flip
            0x00, 0xA1, // set segment remap
            0x00, 0xA8, // set multiplex ratio
            0x00, 0x3F, // set duty 1/64
            0x00, 0xC8, // set com scan direction
            0x00, 0xD3,
            0x00, 0x00, // set display offset
            0x00, 0xD5,
            0x00, 0x80, // set osc division
            0x00, 0xD9,
            0x00, 0xF1, // set pre-charge period
            0x00, 0xDA,
            0x00, 0x12, // set com pins
            0x00, 0xDB,
            0x00, 0x40, // set vcomh
            0x00, 0x8D,
            0x00, 0x14, // set charge pump on
            0x00, 0xA6, // set display normal (not inverse)
            0x00, 0x20,
            0x00, 0x00, // set horizontal addressing mode
            0x00, 0xAF, // set display on
        ],
    )?;

    loop {
        thread::sleep(Duration::from_millis(25));
        let current_time = SystemTime::now();

        // Poll key1
        let key1_value = std::fs::read_to_string("/sys/class/gpio/gpio0/value")?;
        if key1_value.trim() == "1" {
            cmd_index = CMD_INDEX_KEY1;
            display_refresh_time = SystemTime::now();
            continue;
        }

        // Poll key2
        let key2_value = std::fs::read_to_string("/sys/class/gpio/gpio2/value")?;
        if key2_value.trim() == "1" {
            cmd_index = CMD_INDEX_KEY2;
            display_refresh_time = SystemTime::now();
            continue;
        }

        // Poll key3
        let key3_value = std::fs::read_to_string("/sys/class/gpio/gpio3/value")?;
        if key3_value.trim() == "1" {
            cmd_index = CMD_INDEX_KEY3;
            display_refresh_time = SystemTime::now();
            continue;
        }

        if let Ok(elapsed_time) = current_time.duration_since(display_refresh_time) {
            if elapsed_time > Duration::from_secs(0) {
                write_i2c_data(&mut i2c_bus, &[0x00, 0xAF])?; // set display on

                match cmd_index {
                    0 => {
                        cmd_index = CMD_INDEX_KEY1;
                        cmd_index = CMD_INDEX_KEY2;
                        cmd_index = CMD_INDEX_KEY3;

                        let splash = image::open("splash.png").unwrap();
                        let splash_mono = splash.to_mono();
                        image_data.copy_from_slice(splash_mono.as_raw());
                        write_i2c_image_data(&mut i2c_bus, &image_data)?;
                    }
                    1 => {
                        cmd_index = CMD_INDEX_KEY1;
                        cmd_index = CMD_INDEX_KEY2;
                        cmd_index = CMD_INDEX_KEY3;

                        let text1 = chrono::Local::now().format("%A").to_string();
                        let text2 = chrono::Local::now().format("%e %b %Y").to_string();
                        let text3 = chrono::Local::now().format("%X").to_string();

                        let mut image = image::GrayImage::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32);
                        image.fill(image::Luma([0u8]));
                        draw_text(&mut image, &text1, 6, 2, &font15)?;
                        draw_text(&mut image, &text2, 6, 20, &font15)?;
                        draw_text(&mut image, &text3, 6, 36, &font25)?;
                        write_i2c_image_data(&mut i2c_bus, &image)?;
                    }
                    2 => {
                        cmd_index = CMD_INDEX_KEY1;
                        cmd_index = CMD_INDEX_KEY2;
                        cmd_index = CMD_INDEX_KEY3;

                        let text1 = get_command_output("ip a show | grep -E '^\s*inet' | grep -m1 global | awk '{printf \"IPv4: %s\", $2}' | sed 's|/.*||'")?;
                        let text2 = get_command_output("df -h | awk '$NF==\"/\"{printf \"Disk: %d/%dGB %s\", $3,$2,$5}'")?;
                        let text3 = get_command_output("free -m | awk 'NR==2{printf \"RAM:  %s/%sMB\", $3,$2 }'")?;
                        let text4 = get_command_output("top -bn1 | grep 'Cpu' | awk '{printf \"CPU:  %.2f %%\", $(2)}'")?;
                        let text5 = get_command_output("cat /sys/class/thermal/thermal_zone0/temp | awk '{printf \"Temp: %3.1f °C\", $1/1000}'")?;

                        let mut image = image::GrayImage::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32);
                        image.fill(image::Luma([0u8]));
                        draw_text(&mut image, &text1, 6, 2, &font10)?;
                        draw_text(&mut image, &text2, 6, 14, &font10)?;
                        draw_text(&mut image, &text3, 6, 26, &font10)?;
                        draw_text(&mut image, &text4, 6, 38, &font10)?;
                        draw_text(&mut image, &text5, 6, 50, &font10)?;
                        write_i2c_image_data(&mut i2c_bus, &image)?;
                    }
                    3 => {
                        cmd_index = CMD_INDEX_KEY1;
                        cmd_index = CMD_INDEX_KEY2;
                        cmd_index = CMD_INDEX_KEY3;

                        let mut image = image::GrayImage::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32);
                        image.fill(image::Luma([0u8]));
                        draw_text(&mut image, "Shutdown?", 6, 2, &font15)?;
                        draw_rectangle(&mut image, 4, 22, 124, 34)?;
                        draw_text(&mut image, "No", 0, 6, 22, &font10)?;
                        draw_text(&mut image, "Yes", 1, 6, 36, &font10)?;
                        draw_text(&mut image, "F3: Toggle Choices", 1, 6, 54, &font8)?;
                        write_i2c_image_data(&mut i2c_bus, &image)?;
                    }
                    4 => {
                        cmd_index = CMD_INDEX_KEY1;
                        cmd_index = CMD_INDEX_KEY2;
                        cmd_index = CMD_INDEX_KEY3;

                        let mut image = image::GrayImage::new(DISPLAY_WIDTH as u32, DISPLAY_HEIGHT as u32);
                        image.fill(image::Luma([0u8]));
                        draw_text(&mut image, "Shutdown system?", 6, 2, &font15)?;
                        draw_text(&mut image, "Confirm with:", 6, 24, &font10)?;
                        draw_text(&mut image, "F3: Toggle Choices", 1, 6, 54, &font8)?;
                        draw_text(&mut image, "F4: Cancel", 1, 6, 64, &font8)?;
                        write_i2c_image_data(&mut i2c_bus, &image)?;
                    }
                    _ => {
                        cmd_index = CMD_INDEX_KEY1;
                        cmd_index = CMD_INDEX_KEY2;
                        cmd_index = CMD_INDEX_KEY3;
                        write_i2c_data(&mut i2c_bus, &[0x00, 0xAE])?; // set display off
                    }
                }

                display_refresh_time = SystemTime::now();
            }
        }
    }
}

fn draw_text(
    image: &mut image::GrayImage,
    text: &str,
    x: u32,
    y: u32,
    font: &rusttype::Font<'static>,
) -> Result<(), Box<dyn std::error::Error>> {
    let scale = rusttype::Scale::uniform(24.0);
    let v_metrics = font.v_metrics(scale);
    let glyphs: Vec<_> = font.layout(text, scale, rusttype::point(0.0, 0.0)).collect();
    let mut x_pos = x as f32;

    for glyph in glyphs {
        if let Some(bb) = glyph.pixel_bounding_box() {
            glyph.draw(|x, y, v| {
                let px = (x_pos + x as f32 + bb.min.x as f32) as u32;
                let py = (y + bb.min.y as i32 + y as i32 + (v_metrics.ascent * 2.0) as i32) as u32;
                if let Some(p) = image.get_pixel_mut(px, py) {
                    *p = image::Luma([(v * 255.0) as u8]);
                }
            });
        }
        x_pos += glyph.unpositioned().h_metrics().advance_width;
    }

    Ok(())
}

fn draw_rectangle(
    image: &mut image::GrayImage,
    x1: u32,
    y1: u32,
    x2: u32,
    y2: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let color = image::Luma([255u8]);
    for x in x1..=x2 {
        for y in y1..=y2 {
            if let Some(p) = image.get_pixel_mut(x, y) {
                *p = color;
            }
        }
    }
    Ok(())
}

fn get_command_output(command: &str) -> std::io::Result<String> {
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.to_string())
}
