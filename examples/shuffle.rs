use sprunk::RandomMixer;

fn main() {
    let mut mixer = RandomMixer::new(5, 0.2);
    let mut last = None;
    for _ in 0..100 {
        let v = mixer.choose(0..10, |k| k).unwrap();
        println!("{}", v);
        if last == Some(v) {
            println!("REPEAT!");
        }
        last = Some(v);
    }
}
