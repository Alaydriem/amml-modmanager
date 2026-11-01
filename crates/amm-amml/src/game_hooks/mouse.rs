use amml_sdk::amml_detour;

use crate::services::ManagerRuntime;

// void MouseDevice::feed(char actionButtonId, schar buttonData, short x, short y, short dx, short dy,
// bool forceMotionlessPointer): every mouse event of the game's own feed, all arguments by value. A cancelled
// event never reaches the original; otherwise the original gets the position the manager may have moved
amml_detour!(pub mod feed(this: usize, action: u8, data: i8, x: i16, y: i16, dx: i16, dy: i16, motionless: u8) -> () |original| {
    let (mut game_x, mut game_y) = (x, y);
    let cancel = ManagerRuntime::get().is_some_and(|runtime| runtime.mouse(action, &mut game_x, &mut game_y, dx, dy));
    if !cancel {
        original(this, action, data, game_x, game_y, dx, dy, motionless);
    }
});
