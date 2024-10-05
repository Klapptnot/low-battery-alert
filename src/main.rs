use std::fs;
use std::io;
use std::io::Read;
use std::path::Path;
use std::process::Command;

use std::ffi::CString;

use raylib::consts::MouseButton::MOUSE_BUTTON_LEFT;
use raylib::ffi;
use raylib::ffi::MeasureTextEx;
use raylib::prelude::*;

const BATTERY_LEVEL_LOW: i32 = 20;
const BATTERY_LEVEL_CRITICAL: i32 = 10;

#[derive(Debug)]
struct BatteryStatus {
  level: i32,
  is_charging: bool,
}

fn update_battery_info(battery_status: &mut BatteryStatus) -> io::Result<()> {
  battery_status.level = -1;
  battery_status.is_charging = false;

  let mut battery_capacity_path: Option<String> = None;
  let mut battery_status_path: Option<String> = None;

  let dir = fs::read_dir("/sys/class/power_supply")?;
  for entry in dir {
    let entry = entry?;
    let mut path = entry.path();

    if path.is_dir() && path.file_name().unwrap().to_str().unwrap().starts_with("BAT") {
      let mut cppath = path.clone();
      cppath.push("capacity");
      battery_capacity_path = Some(cppath.into_os_string().into_string().unwrap());
      path.push("status");
      battery_status_path = Some(path.into_os_string().into_string().unwrap());
      break;
    }
  }

  let mut fp = fs::OpenOptions::new()
    .read(true)
    .open(battery_capacity_path.unwrap())?;

  let mut buffer = String::new();
  fp.read_to_string(&mut buffer)?;
  battery_status.level = buffer.trim().parse::<i32>().unwrap_or(-1);

  let mut fp = fs::OpenOptions::new()
    .read(true)
    .open(battery_status_path.unwrap())?;
  buffer.clear();
  fp.read_to_string(&mut buffer)?;
  battery_status.is_charging = buffer.trim() == "Charging";

  Ok(())
}

fn hibernate_machine() -> Result<(), std::io::Error> {
  let output = Command::new("systemctl").arg("hibernate").output()?;
  if output.status.success() {
    Ok(())
  } else {
    Err(std::io::Error::new(
      std::io::ErrorKind::Other,
      format!("Failed to hibernate machine: {}", output.status),
    ))
  }
}

fn search_for_font(font_name: &str) -> Option<String> {
  let local_font_dir = Path::new("~/.local/share/fonts");
  let system_font_dir = Path::new("/usr/share/fonts");

  let ext = Path::new(&font_name).extension().and_then(|s| s.to_str());
  match ext {
    Some("ttf") | Some("otf") | Some("woff") | Some("woff2") => return Some(font_name.into()),
    _ => (),
  };

  let font_name = format!("{}.ttf", font_name);

  if local_font_dir.exists() {
    let local_font_path = local_font_dir.join(&font_name);
    if local_font_path.exists() {
      return Some(local_font_path.to_str().unwrap().to_string());
    }
  }

  if system_font_dir.exists() {
    let system_font_path = system_font_dir.join(&font_name);
    if system_font_path.exists() {
      return Some(system_font_path.to_str().unwrap().to_string());
    }
  }

  None
}

fn main() -> io::Result<()> {
  const SLEEP_TIME: std::time::Duration = std::time::Duration::from_secs(1);
  let mut args = std::env::args().take(2);
  args.next(); // consume self name

  let argv1 = args.next();
  if argv1.is_none() {
    eprintln!("Expected a font file path");
    std::process::exit(1)
  };
  let font = search_for_font(argv1.unwrap().as_str().as_ref()).unwrap();
  let font = font.as_str();

  let hib_button: Rectangle = Rectangle {
    x: 15f32,
    y: 100f32,
    width: 220f32,
    height: 40f32,
  };
  let ign_button: Rectangle = Rectangle {
    x: 245f32,
    y: 100f32,
    width: 220f32,
    height: 40f32,
  };
  let btn_colorn = rcolor(18, 18, 18, 255);
  let btn_colorh = rcolor(38, 38, 38, 255);

  let mut bss: BatteryStatus = BatteryStatus {
    level: -1,
    is_charging: false,
  };

  let mut showed_critical = false;
  let mut showed_low = false;
  let mut level_critical = false;
  loop {
    std::thread::sleep(SLEEP_TIME);
    update_battery_info(&mut bss)?;

    if bss.is_charging {
      showed_low = false;
      showed_critical = false;
      level_critical = false;
      continue;
    }

    if (showed_low && bss.level > BATTERY_LEVEL_CRITICAL) || showed_critical {
      continue;
    }

    if bss.level > BATTERY_LEVEL_CRITICAL {
      if bss.level > BATTERY_LEVEL_LOW {
        continue;
      }
    }

    // Reach here if is needed to show a window
    let (mut rh, rthread) = raylib::init().size(480, 160).title("Low battery alert").build();

    unsafe {
      ffi::SetConfigFlags(
        ffi::ConfigFlags::FLAG_WINDOW_TOPMOST as u32
          | ffi::ConfigFlags::FLAG_WINDOW_UNDECORATED as u32
          | ffi::ConfigFlags::FLAG_WINDOW_HIGHDPI as u32,
      );
    }

    let font28 = rh.load_font_ex(&rthread, &font, 28, None).expect("load a font");
    let font20 = rh.load_font_ex(&rthread, &font, 20, None).expect("load a font");
    let text_spacing = 0.0;

    let btn_text_size = unsafe {
      MeasureTextEx(
        *font20,
        CString::new("Got it").expect("cstring").into_raw(),
        20.0,
        text_spacing,
      )
    };
    let hib_text_size = unsafe {
      MeasureTextEx(
        *font20,
        CString::new("Hibernate").expect("cstring").into_raw(),
        20.0,
        text_spacing,
      )
    };
    let hib_text_pos = 125f32 - (hib_text_size.x / 2.0);
    let ign_text_pos = 355f32 - (btn_text_size.x / 2.0);

    if bss.level <= BATTERY_LEVEL_CRITICAL {
      showed_critical = true;
      level_critical = true;
    }
    showed_low = true;
    loop {
      if rh.window_should_close() || bss.is_charging {
        break;
      }
      if bss.level <= BATTERY_LEVEL_CRITICAL {
        level_critical = true;
      }

      let mut drw = rh.begin_drawing(&rthread);

      drw.clear_background(Color::BLACK);
      // Title
      let msg_title = if level_critical {
        "Battery critically low"
      } else {
        "Battery low"
      };
      drw.draw_text_ex(
        &font28,
        msg_title.as_ref(),
        Vector2::new(18f32, 18f32),
        28.0,
        text_spacing,
        Color::WHITE,
      );
      // Body
      let msg_body = &format!("Battery at {}%. You might want to plug in your PC", bss.level);
      drw.draw_text_ex(
        &font20,
        msg_body.as_str().as_ref(),
        Vector2::new(18f32, 52f32),
        20.0,
        text_spacing,
        Color::WHITE,
      );

      let mouse_pos: Vector2 = unsafe { ffi::GetMousePosition().into() };
      let ign_btn_hover = ign_button.check_collision_point_rec(mouse_pos);
      let hib_btn_hover = hib_button.check_collision_point_rec(mouse_pos);
      let mouse_pressed = unsafe { ffi::IsMouseButtonDown(MOUSE_BUTTON_LEFT as i32) };

      if level_critical {
        if hib_btn_hover {
          drw.draw_rectangle_rounded(hib_button, 16.0, 2, btn_colorh);
        } else {
          drw.draw_rectangle_rounded(hib_button, 16.0, 2, btn_colorn);
        }

        drw.draw_text_ex(
          &font20,
          "Hibernate",
          Vector2::new(hib_text_pos, 110f32),
          20.0,
          text_spacing,
          Color::WHITE,
        );
      }

      if ign_btn_hover {
        drw.draw_rectangle_rounded(ign_button, 16.0, 2, btn_colorh);
      } else {
        drw.draw_rectangle_rounded(ign_button, 16.0, 2, btn_colorn);
      }

      drw.draw_text_ex(
        &font20,
        "Got it",
        Vector2::new(ign_text_pos, 110f32),
        20.0,
        text_spacing,
        Color::WHITE,
      );

      if hib_btn_hover && mouse_pressed {
        break hibernate_machine().expect("hibernate machine");
      }
      if ign_btn_hover && mouse_pressed {
        break;
      }
      update_battery_info(&mut bss)?;
    }
    rh.unload_font(font28.make_weak());
    rh.unload_font(font20.make_weak());
  }
}
