#![no_main]

use jpeg2000_decoder::mq::MqDecoder;
use jpeg2000_decoder_fuzz::resource_limits;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let limits = resource_limits();
    let payload = data.get(1..).unwrap_or_default();
    let Ok(mut decoder) = MqDecoder::new(payload, &limits) else {
        return;
    };
    if let Some(control) = data.first().copied() {
        let context = usize::from(control % 19);
        let state = control % 47;
        let _ = decoder.set_context(context, state, control & 0x80 != 0);
        let decisions = usize::from(control);
        for index in 0..decisions {
            if decoder.decode(index % 19).is_err() {
                break;
            }
        }
    }
});
