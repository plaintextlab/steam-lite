//! Decode cover art off the main thread, at tile size, with a small cache.

use gtk::{gdk, gdk::gdk_pixbuf::Pixbuf, gio, glib};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;

/// Parallel decodes; covers are small so two workers keep up easily.
const WORKERS: usize = 2;

type Callback = Box<dyn FnOnce(&gdk::Texture)>;

struct Decoded {
    bytes: glib::Bytes,
    width: i32,
    height: i32,
    stride: usize,
    alpha: bool,
}

#[derive(Default)]
pub struct ArtLoader {
    cache: RefCell<HashMap<u32, gdk::Texture>>,
    queue: RefCell<VecDeque<(u32, PathBuf, i32, i32, Callback)>>,
    active: Cell<usize>,
}

fn decode(path: PathBuf, w: i32, h: i32) -> Option<Decoded> {
    // libjpeg scales during decode, so this never holds a full-size image.
    let pb = Pixbuf::from_file_at_scale(&path, w, h, true).ok()?;
    Some(Decoded {
        width: pb.width(),
        height: pb.height(),
        stride: pb.rowstride() as usize,
        alpha: pb.has_alpha(),
        bytes: pb.read_pixel_bytes(),
    })
}

impl ArtLoader {
    /// Call `done` with the texture for `appid`, decoding `path` scaled to
    /// fit `w`×`h` device pixels if it isn't cached yet.
    pub fn request(self: &Rc<Self>, appid: u32, path: PathBuf, w: i32, h: i32, done: Callback) {
        if let Some(t) = self.cache.borrow().get(&appid) {
            done(t);
            return;
        }
        self.queue.borrow_mut().push_back((appid, path, w, h, done));
        self.pump();
    }

    /// Drop queued (not yet started) requests, e.g. before a rescan.
    pub fn cancel_pending(&self) {
        self.queue.borrow_mut().clear();
    }

    /// Forget decoded textures (tile size changed).
    pub fn clear(&self) {
        self.cancel_pending();
        self.cache.borrow_mut().clear();
    }

    fn pump(self: &Rc<Self>) {
        while self.active.get() < WORKERS {
            let Some((appid, path, w, h, done)) = self.queue.borrow_mut().pop_front() else { return };
            self.active.set(self.active.get() + 1);
            let this = self.clone();
            glib::spawn_future_local(async move {
                let res = gio::spawn_blocking(move || decode(path, w, h)).await.ok().flatten();
                if let Some(d) = res {
                    let fmt = if d.alpha { gdk::MemoryFormat::R8g8b8a8 } else { gdk::MemoryFormat::R8g8b8 };
                    let tex: gdk::Texture =
                        gdk::MemoryTexture::new(d.width, d.height, fmt, &d.bytes, d.stride).into();
                    this.cache.borrow_mut().insert(appid, tex.clone());
                    done(&tex);
                }
                this.active.set(this.active.get() - 1);
                this.pump();
            });
        }
    }
}
