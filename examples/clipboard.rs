use moving::clipboard::{self, mime};

fn main() {
    if let Some(paste) = clipboard::load(mime::TEXT_PLAIN).expect("Failed to paste") {
        println!("Clipboard: {}", String::from_utf8_lossy(&paste));
    }
    else {
        println!("The clipboard is empty");
    }
}
