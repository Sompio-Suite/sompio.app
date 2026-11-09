fn main() {
    let s = sompio_lock::PictureSecret::new().unwrap();
    let png = sompio_lock::make_picture(&s).unwrap();
    std::fs::write(std::env::args().nth(1).unwrap(), png).unwrap();
}
