use amm_core::errors::StoreError;
use amm_core::structs::capture::{CaptureImage, PixelFormat};
use amm_core::structs::write::WriteRequest;
use amm_store::services::IoQueue;
use amm_store::structs::IoJob;

fn request(index: usize) -> WriteRequest {
    WriteRequest { path: format!("c{index}.hcl").into(), text: String::new() }
}

#[test]
fn queue_past_capacity_refuses_and_counts() {
    let queue = IoQueue::new(32);

    for index in 0..32 {
        queue.try_submit(request(index)).unwrap();
    }
    let result = queue.try_submit(request(32));

    assert_eq!(result, Err(StoreError::QueueFull));
    assert_eq!(queue.refused(), 1);
    assert_eq!(queue.receiver().len(), 32);
    let Ok(IoJob::Config(first)) = queue.receiver().try_recv() else { panic!("a config write comes first") };
    assert_eq!(first.path, std::path::PathBuf::from("c0.hcl"));
}

#[test]
fn closed_queue_refuses_without_counting_a_full_queue() {
    let queue = IoQueue::new(4);
    queue.close();

    assert_eq!(queue.try_submit(request(0)), Err(StoreError::Unavailable));
    assert_eq!(queue.refused(), 0);
}

#[test]
fn a_capture_past_capacity_is_refused_and_counted() {
    let queue = IoQueue::new(1);
    let image = CaptureImage {
        sequence: 1,
        path: "shot.png".into(),
        pixels: vec![0; 4],
        width: 1,
        height: 1,
        row_pitch: 4,
        format: PixelFormat::DXGI_R8G8B8A8_UNORM,
    };

    queue.try_submit(request(0)).unwrap();
    let result = queue.try_submit_capture(image);

    assert_eq!(result, Err(StoreError::QueueFull));
    assert_eq!(queue.refused(), 1);
}
