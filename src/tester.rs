/// Small binary that has an non-trivial trace
use std::{thread::sleep, time::Duration};

#[inline(never)]
fn a() {
    println!("in a");
    for _i in 0..10 {
        sleep(Duration::from_millis(100));
    }
    b();
}

#[inline(never)]
fn b() {
    println!("in b");
    for _i in 0..10 {
        sleep(Duration::from_millis(100));
    }
}

fn c() {
    println!("in c");
    for _i in 0..10 {
        sleep(Duration::from_millis(100));
    }
    b();
}

fn main() {
    a();
    b();
    c();
}
