//! The small helper browsers start for the Sompio extension (native messaging). A console
//! program, as browsers expect; it never shows a window. See the library for what it does.

fn main() {
    sompio_browser_host::run();
}
