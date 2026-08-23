use super::*;
use icy_parser_core::{AnsiParser, CommandParser};

#[test]
fn test_dcs_error() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // DCS with unknown content now reports error instead of Unknown
    parser.parse(b"\x1BPHello\x1B\\World", &mut sink);
    assert_eq!(sink.cmds.len(), 0); // No commands emitted for malformed DCS
    assert_eq!(sink.text, b"World");

    sink.text.clear();

    // DCS with ESC in the middle (not a terminator) also reports error
    parser.parse(b"\x1BPTest\x1BData\x1B\\", &mut sink);
    assert_eq!(sink.cmds.len(), 0); // No commands emitted for malformed DCS

    sink.text.clear();
}

#[test]
fn test_dcs_sixel() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // DCS for sixel graphics
    parser.parse(b"\x1BP0;0;8q\"1;1;80;80#0;2;0;0;0#1!80~-#1!80~-\x1B\\", &mut sink);
    assert_eq!(sink.dcs_commands.len(), 1);
    if let DeviceControlString::Sixel {
        aspect_ratio: _,
        zero_color: _,
        grid_size: _,
        sixel_data,
    } = &sink.dcs_commands[0]
    {
        // TODO: Update these assertions based on actual parameter parsing
        assert!(sixel_data.starts_with(b"\"1;1;80;80"));
    } else {
        panic!("Expected Sixel");
    }
}

#[test]
fn test_dcs_hex_macro_repeat_capped() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // Macro definition (!z) with usize::MAX repeat in hex encoding.
    // Without the cap this would allocate ~18 exabytes and OOM.
    parser.parse(b"\x1bP0;0;1!z!18446744073709551615;41;\x1b\\", &mut sink);
}

#[test]
fn test_nested_dec_macros_parse_from_default_state() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"\x1bP1;0;1!z41\x1b\\", &mut sink);
    parser.parse(b"\x1bP2;0;1!z5B1B5B312A7A5D\x1b\\", &mut sink);
    parser.parse(b"\x1b[2*z", &mut sink);

    assert_eq!(sink.text, b"[A]");
}

#[test]
fn test_self_referential_macro_does_not_recurse() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // Macro 0 body is ESC [ 0 * z, i.e. it invokes itself.
    parser.parse(b"\x1bP0;0;1!z411B5B302A7A\x1b\\", &mut sink);
    parser.parse(b"\x1b[0*z", &mut sink);

    // The nested invocation is refused, so the body runs exactly once.
    assert_eq!(sink.text, b"A");
}

#[test]
fn test_mutually_recursive_macros_do_not_recurse() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // Macro 1 prints "A" then invokes macro 2; macro 2 prints "B" then invokes macro 1.
    parser.parse(b"\x1bP1;0;1!z411B5B322A7A\x1b\\", &mut sink);
    parser.parse(b"\x1bP2;0;1!z421B5B312A7A\x1b\\", &mut sink);
    parser.parse(b"\x1b[1*z", &mut sink);

    assert_eq!(sink.text, b"AB");
}

#[test]
fn test_macro_slots_above_63_are_ignored() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    parser.parse(b"\x1bP64;0;1!z41\x1b\\", &mut sink);
    parser.parse(b"\x1b[64*z", &mut sink);

    assert!(sink.text.is_empty());
}

#[test]
fn test_dcs_font_loading() {
    let mut parser = AnsiParser::new();
    let mut sink = CollectSink::new();

    // DCS for custom font loading: CTerm:Font:{slot}:{base64_data}
    // Base64 "dGVzdGRhdGE=" decodes to "testdata"
    parser.parse(b"\x1BPCTerm:Font:5:dGVzdGRhdGE=\x1B\\", &mut sink);
    assert_eq!(sink.dcs_commands.len(), 1);
    if let DeviceControlString::LoadFont(slot, data) = &sink.dcs_commands[0] {
        assert_eq!(slot, &5);
        assert_eq!(data, b"testdata");
    } else {
        panic!("Expected LoadFont");
    }
}
