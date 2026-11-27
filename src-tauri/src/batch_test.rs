#[cfg(all(test, windows))]
#[test]
fn batch_valid_corrupt_valid() {
    use crate::commands::{batch_upscale, AppState, BatchItemDto, BatchProgress, BatchUpscaleDto};
    use std::sync::{Arc, Mutex};
    use tauri::ipc::Channel;
    use tauri::Manager;
    let tmp = std::env::temp_dir().join("cookiesincraft_batch");
    let temp_dir = tmp.join(format!("{}_{}", std::process::id(), fastrand::u64(..)));
    std::fs::create_dir_all(&temp_dir).expect("failed to create temp dir");
    let valid_png1 = temp_dir.join("valid1.png");
    let valid_png2 = temp_dir.join("valid2.png");
    {
        let img1 = image::RgbaImage::from_pixel(1, 1, image::Rgba([255u8, 0, 0, 255]));
        img1.save(&valid_png1).expect("png write infallible");
    }
    {
        let img2 = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 255, 0, 255]));
        img2.save(&valid_png2).expect("png write infallible");
    }
    let corrupt_png = temp_dir.join("corrupt.png");
    std::fs::write(&corrupt_png, b"not a png file at all!!!").expect("write failed");
    let mut context = tauri::generate_context!();
    context.config_mut().app.windows.clear();
    let app = tauri::Builder::default()
        .any_thread()
        .manage(AppState::isolated_for_tests())
        .build(context)
        .expect("build tauri app");
    let state = app.state::<AppState>();
    state.allow_read(valid_png1.to_str().unwrap());
    state.allow_read(valid_png2.to_str().unwrap());
    state.allow_read(corrupt_png.to_str().unwrap());
    let events = Arc::new(Mutex::new(Vec::<BatchProgress>::new()));
    let recv = Arc::clone(&events);
    let chan = Channel::<BatchProgress>::new(move |body| {
        let tauri::ipc::InvokeResponseBody::Json(raw) = body else {
            panic!("expected JSON")
        };
        let progress: BatchProgress = serde_json::from_str(&raw).unwrap();
        recv.lock().unwrap().push(progress);
        Ok(())
    });
    let dto = BatchUpscaleDto {
        items: vec![
            BatchItemDto {
                input_path: valid_png1.to_str().unwrap().to_string(),
                output_path: Some(
                    valid_png1
                        .with_extension("upscaled.png")
                        .to_str()
                        .unwrap()
                        .to_string(),
                ),
            },
            BatchItemDto {
                input_path: corrupt_png.to_str().unwrap().to_string(),
                output_path: Some(
                    corrupt_png
                        .with_extension("upscaled.png")
                        .to_str()
                        .unwrap()
                        .to_string(),
                ),
            },
            BatchItemDto {
                input_path: valid_png2.to_str().unwrap().to_string(),
                output_path: Some(
                    valid_png2
                        .with_extension("upscaled.png")
                        .to_str()
                        .unwrap()
                        .to_string(),
                ),
            },
        ],
        backend: "cpu".to_string(),
        algorithm: "nearest".to_string(),
        keep_aspect_ratio: false,
        target_width: 5,
        target_height: 3,
        quality: None,
    };
    let result = tauri::async_runtime::block_on(batch_upscale(
        dto,
        chan,
        app.state::<AppState>().clone(),
        app.handle().clone(),
    ));
    let result_dto = result.expect("batch_upscale should return Ok");
    assert_eq!(result_dto.total, 3);
    let event_seq = events.lock().unwrap();
    let tags: Vec<&str> = event_seq
        .iter()
        .map(|v| match v {
            BatchProgress::Started { .. } => "Started",
            BatchProgress::Progress { .. } => "Progress",
            BatchProgress::ItemCompleted { .. } => "ItemCompleted",
            BatchProgress::ItemFailed { .. } => "ItemFailed",
            BatchProgress::Completed { .. } => "Completed",
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(
        tags,
        vec![
            "Started",
            "Progress",
            "ItemCompleted",
            "Progress",
            "ItemFailed",
            "Progress",
            "ItemCompleted",
            "Completed"
        ]
    );
    let successes: usize = event_seq
        .iter()
        .filter_map(|v| match v {
            BatchProgress::Completed { successes, .. } => Some(*successes),
            _ => None,
        })
        .sum();
    let failures: usize = event_seq
        .iter()
        .filter_map(|v| match v {
            BatchProgress::Completed { failures, .. } => Some(*failures),
            _ => None,
        })
        .sum();
    assert_eq!(successes, 2);
    assert_eq!(failures, 1);
    assert_eq!(successes + failures, result_dto.total);
    let out1_path = valid_png1.with_extension("upscaled.png");
    let out2_path = valid_png2.with_extension("upscaled.png");
    let corrupt_out_path = corrupt_png.with_extension("upscaled.png");
    assert!(
        out1_path.exists() && out2_path.exists(),
        "2 valid outputs should exist"
    );
    assert!(
        !corrupt_out_path.exists(),
        "corrupt output should not exist"
    );
    let out1_img = image::open(&out1_path).expect("failed to open").to_rgba8();
    let out2_img = image::open(&out2_path).expect("failed to open").to_rgba8();
    assert_eq!(out1_img.dimensions(), (5, 3));
    assert_eq!(out2_img.dimensions(), (5, 3));
    assert_eq!(*out1_img.get_pixel(0, 0), image::Rgba([255u8, 0, 0, 255]));
    assert_eq!(*out2_img.get_pixel(0, 0), image::Rgba([0, 255, 0, 255]));
    assert!(
        !state.is_running(&result_dto.batch_id),
        "registry should be removed"
    );
    let _ = std::fs::remove_dir_all(&temp_dir);
}
