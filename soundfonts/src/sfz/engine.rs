use std::collections::{HashSet, HashMap};
use std::convert::TryFrom;

use itertools::izip;

use crate::errors::*;
use crate::engine;
use crate::sample;
use crate::envelopes;
use crate::utils;

#[derive(Clone, Copy)]
pub(super) struct VelRange {
    lo: wmidi::Velocity,
    hi: wmidi::Velocity
}

impl VelRange {
    pub(super) fn set_hi(&mut self, v: i32) -> Result<(), RangeError> {
	let vel = wmidi::Velocity::try_from(v as u8).map_err(|_| RangeError::out_of_range("hivel", 0, 127, v))?;
	if  vel < self.lo {
	    return Err(RangeError::flipped_range("hivel", v, u8::from(self.lo) as i32));
	}
	self.hi = vel;
	Ok(())
    }

    pub(super) fn set_lo(&mut self, v: i32) -> Result<(), RangeError> {
	let vel = wmidi::Velocity::try_from(v as u8).map_err(|_| RangeError::out_of_range("lovel", 0, 127, v))?;
	if  vel > self.hi {
	    return Err(RangeError::flipped_range("lovel", v, u8::from(self.hi) as i32));
	}
	self.lo = vel;
	Ok(())
    }

    pub(super) fn covering(&self, vel: wmidi::Velocity) -> bool {
	vel >= self.lo && vel <= self.hi
    }
}


impl Default for VelRange {
    fn default() -> Self {
	VelRange {
	    hi: wmidi::Velocity::MAX,
	    lo: wmidi::Velocity::MIN
	}
    }
}

#[derive(Clone, Copy)]
pub(super) struct NoteRange {
    lo: Option<wmidi::Note>,
    hi: Option<wmidi::Note>
}

impl NoteRange {
    pub(super) fn set_hi(&mut self, v: i32) -> Result<(), RangeError> {
	match v {
	    -1 => {
		self.hi = None;
		Ok(())
	    }
	    v if v < 0 && v > 127 => Err(RangeError::out_of_range("hikey", -1, 127, v)),
	    _ => {
		let note = unsafe { wmidi::Note::from_u8_unchecked(v as u8) };
		if self.lo.map_or(false, |n| note < n) {
		    return Err(RangeError::flipped_range("hikey", v, u8::from(note) as i32));
		}
		self.hi = Some(note);
		Ok(())
	    }
	}
    }

    pub(super) fn set_lo(&mut self, v: i32) -> Result<(), RangeError> {
	match v {
	    -1 => {
		self.lo = None;
		Ok(())
	    }
	    v if v > 127 => Err(RangeError::out_of_range("lokey", -1, 127, v)),
	    _ => {
		let note = unsafe { wmidi::Note::from_u8_unchecked(v as u8) };
		if self.hi.map_or(false, |n| note > n) {
		    return Err(RangeError::flipped_range("lokey", v, u8::from(note) as i32));
		}
		self.lo = Some(note);
		Ok(())
	    }
	}
    }

    pub(super) fn covering(&self, note: wmidi::Note) -> bool {
	match (self.lo, self.hi) {
	    (Some(lo), Some(hi)) => note >= lo && note <= hi,
	     _ => false
	}
    }
}


impl Default for NoteRange {
    fn default() -> Self {
	NoteRange {
	    hi: Some(wmidi::Note::HIGHEST_NOTE),
	    lo: Some(wmidi::Note::LOWEST_NOTE)
	}
    }
}


#[derive(Default, Clone)]
pub(super) struct RandomRange {
    hi: f32,
    lo: f32
}

impl RandomRange {
    pub(super) fn set_hi(&mut self, v: f32) -> Result<(), RangeError> {
	match v {
	    v if v < 0.0 && v > 1.0 => Err(RangeError::out_of_range("hirand", "0.0", "1.0", v.to_string().as_str())),
	    v if v < self.lo && self.lo > 0.0 => Err(RangeError::flipped_range("hirand", v.to_string().as_str(), self.lo.to_string().as_str())),
	    _ => {
		self.hi = v;
		Ok(())
	    }
	}
    }

    pub(super) fn set_lo(&mut self, v: f32) -> Result<(), RangeError> {
	match v {
	    v if v < 0.0 && v > 1.0 => Err(RangeError::out_of_range("lorand", 0.0, 1.0, v)),
	    v if v > self.hi && self.hi > 0.0 => Err(RangeError::flipped_range("lorand", v, self.hi)),
	    _ => {
		self.lo = v;
		Ok(())
	    }
	}
    }
}

#[derive(Default, Clone)]
pub(super) struct ControlValRange {
    hi: Option<wmidi::ControlValue>,
    lo: Option<wmidi::ControlValue>
}

impl ControlValRange {
    pub(super) fn set_hi(&mut self, v: i32) -> Result<(), RangeError> {
	if v < 0 {
	    self.hi = None;
	    return Ok(());
	}
	let val = wmidi::ControlValue::try_from(v as u8).map_err(|_| RangeError::out_of_range("on_hiccXX", 0, 127, v))?;
	match self.lo {
	    Some(lo) if val < lo => {
		return Err(RangeError::flipped_range("on_hiccXX", v, u8::from(lo) as i32));
	    }
	    _ => {}
	};
	self.hi = Some(val);
	Ok(())
    }

    pub(super) fn set_lo(&mut self, v: i32) -> Result<(), RangeError> {
	if v < 0 {
	    self.lo = None;
	    return Ok(());
	}
	let val = wmidi::ControlValue::try_from(v as u8).map_err(|_| RangeError::out_of_range("on_loccXX", 0, 127, v))?;
	match self.hi {
	    Some(hi) if val > hi => {
		return Err(RangeError::flipped_range("on_loccXX", v, u8::from(hi) as i32));
	    }
	    _ => {}
	};
	self.lo = Some(val);
	Ok(())
    }

    pub(super) fn covering(&self, vel: wmidi::ControlValue) -> bool {
	match (self.lo, self.hi) {
	    (Some(lo), Some(hi)) => vel >= lo && vel <= hi,
	     _ => false
	}
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Trigger {
    Attack,
    Release,
    First,
    Legato,
    ReleaseKey
}

impl Default for Trigger {
    fn default() -> Self {
	Trigger::Attack
    }
}



#[derive(Clone)]
pub struct RegionData {
    pub(super) key_range: NoteRange,
    pub(super) vel_range: VelRange,

    pub(super) ampeg: envelopes::Generator,

    pitch_keycenter: wmidi::Note,

    pitch_keytrack: f64,

    amp_veltrack: f32,

    volume: f32,

    sample: String,
    rt_decay: f32,

    tune: f64,

    trigger: Trigger,

    group: u32,
    off_by: u32,

    on_ccs: HashMap<u8, ControlValRange>,

    pub(super) random_range: RandomRange,
}


impl Default for RegionData {
    fn default() -> Self {
	RegionData {
	    key_range: Default::default(),
	    vel_range: Default::default(),

	    pitch_keycenter: wmidi::Note::C3,

	    pitch_keytrack: 1.0,

	    amp_veltrack: 1.0,

	    ampeg: Default::default(),

	    volume: Default::default(),
	    sample: Default::default(),
	    rt_decay: Default::default(),
	    tune: Default::default(),
	    trigger: Default::default(),

	    group:  Default::default(),
	    off_by:  Default::default(),

	    on_ccs: HashMap::new(),

	    random_range: Default::default(),
	}
    }
}

impl RegionData {
    pub(super) fn set_amp_veltrack(&mut self, v: f32) -> Result<(), RangeError> {
	self.amp_veltrack = range_check(v, -100.0, 100.0, "amp_veltrack")? / 100.0;
	Ok(())
    }

    pub(super) fn set_pitch_keycenter(&mut self, v: u32) -> Result<(), RangeError> {
	let v = range_check(v, 0, 127, "pich_keycenter")? as u8;
	self.pitch_keycenter = unsafe { wmidi::Note::from_u8_unchecked(v as u8) };
	Ok(())
    }

    pub(super) fn set_pitch_keytrack(&mut self, v: f32) -> Result<(), RangeError> {
	self.pitch_keytrack = range_check(v as f64, -1200.0, 1200.0, "pitch_keytrack")? / 100.0;
	Ok(())
    }

    pub(super) fn set_sample(&mut self, v: &str) {
	self.sample = v.to_string();
    }

    pub(super) fn set_rt_decay(&mut self, v: f32) -> Result<(), RangeError> {
	self.rt_decay = range_check(v, 0.0, 200.0, "rt_decay")?;
	Ok(())
    }

    pub(super) fn set_tune(&mut self, v: i32) -> Result<(), RangeError> {
	self.tune = range_check(v, -100, 100, "tune")? as f64 / 100.0;
	Ok(())
    }

    pub(super) fn set_volume(&mut self, v: f32) -> Result<(), RangeError> {
	self.volume = range_check(v, -144.6, 6.0, "tune")?;
	Ok(())
    }

    pub(super) fn set_trigger(&mut self, t: Trigger) {
	self.trigger = t;
    }

    pub(super) fn set_group(&mut self, v: u32) {
	self.group = v;
    }

    pub(super) fn set_off_by(&mut self, v: u32) {
	self.off_by = v;
    }

    pub(super) fn push_on_lo_cc(&mut self, channel: u32, v: i32) -> Result<(), RangeError> {
	let channel = channel as u8;
	match self.on_ccs.get_mut(&channel) {
	    Some(ref mut range) => range.set_lo(v),
	    None => {
		let mut range = ControlValRange { hi: None, lo: None };
		range.set_lo(v)?;
		self.on_ccs.insert(channel, range);
		Ok(())
	    }
	}
    }

    pub(super) fn push_on_hi_cc(&mut self, channel: u32, v: i32) -> Result<(), RangeError> {
	let channel = channel as u8;
	match self.on_ccs.get_mut(&channel) {
	    Some(ref mut range) => range.set_hi(v),
	    None => {
		let mut range = ControlValRange { hi: None, lo: None };
		range.set_hi(v)?;
		self.on_ccs.insert(channel, range);
		Ok(())
	    }
	}
    }

}


pub(super) struct Region {
    params: RegionData,

    sample: sample::Sample,

    amp_envelope: envelopes::ADSREnvelope,

    gain: f32,

    samplerate: f64,
    real_sample_length: usize,
    max_block_length: usize,

    current_note_frequency: f64,

    last_note_on: Option<(wmidi::Note, wmidi::Velocity)>,
    other_notes_on: HashSet<u8>,

    sustain_pedal_pushed: bool
}

impl Region {
    fn new(params: RegionData, samplerate: f64, max_block_length: usize) -> Region {

	let amp_envelope = envelopes::ADSREnvelope::new(&params.ampeg, samplerate as f32, max_block_length);

	Region {
	    params: params,

	    // FIXME: should be initialized
	    sample: sample::Sample::new(Vec::new(), max_block_length, 440.0),

	    gain: 1.0,

	    amp_envelope: amp_envelope,

	    samplerate: samplerate,
	    max_block_length: max_block_length,
	    real_sample_length: 0,

	    current_note_frequency: 0.0,

	    last_note_on: None,
	    other_notes_on: HashSet::new(),

	    sustain_pedal_pushed: false
	}
    }

    // # should be done in ::new()
    fn set_sample_data(&mut self, sample_data: Vec<f32>) {
	self.sample = sample::Sample::new(sample_data, self.max_block_length, self.params.pitch_keycenter.to_freq_f64());
    }

    fn process(&mut self, out_left: &mut [f32], out_right: &mut [f32]) {
	if !(self.sample.is_playing() && self.amp_envelope.is_playing_or_releasing()) {
	    return;
	}

	let (envelope, mut env_position) = self.amp_envelope.active_envelope();

	let sample_iterator = self.sample.iter(self.current_note_frequency);

	for (l, r, (sl, sr)) in izip!(out_left.iter_mut(), out_right.iter_mut(), sample_iterator) {
	    *l += sl * self.gain * envelope[env_position];
	    *r += sr * self.gain * envelope[env_position];

	    env_position += 1;
	}

	self.amp_envelope.update(env_position);

    }

    fn is_active(&self) -> bool {
	self.sample.is_playing() && self.amp_envelope.is_playing()
    }

    fn note_on(&mut self, note: wmidi::Note, velocity: wmidi::Velocity) {
	if self.is_active() {
	    return;
	}

	let velocity = u8::from(velocity);
	let vel = if self.params.amp_veltrack < 0.0 {
	    127 - velocity
	} else {
	    velocity
	};

	let velocity_db = if vel == 0 {
	    -160.0
	} else {
	    let vel = vel as f32;
	    -20.0 * ((127.0 * 127.0)/(vel * vel)).log10()
	};
	self.gain = utils::dB_to_gain(self.params.volume + velocity_db * self.params.amp_veltrack.abs());

	let native_freq = self.params.pitch_keycenter.to_freq_f64();

	self.current_note_frequency = native_freq * (note.to_freq_f64()/native_freq).powf(self.params.pitch_keytrack) * 2.0f64.powf(1.0/12.0 * self.params.tune);

	self.sample.note_on();
	self.amp_envelope.note_on();

    }

    fn note_off(&mut self) {
	if self.is_active() {
	    self.amp_envelope.note_off();
	}
    }

    fn sustain_pedal(&mut self, pushed: bool) {
	self.sustain_pedal_pushed = pushed;

	if !pushed {
	    match self.params.trigger {
		Trigger::Release => self.last_note_on.map_or((), |(note, velocity)| self.note_on(note, velocity)),
		_ => self.note_off()
	    }
	}
    }

    fn handle_note_on(&mut self, note: wmidi::Note, velocity: wmidi::Velocity) {
	if !self.params.key_range.covering(note) {
	    self.other_notes_on.insert(u8::from(note));
	    return;
	}

	if !self.params.vel_range.covering(velocity) {
	    return;
	}

 	match self.params.trigger {
	    Trigger::Release |
	    Trigger::ReleaseKey => {
		self.last_note_on = Some((note, velocity));
		return
	    }
	    Trigger::First => {
		if !self.other_notes_on.is_empty() {
		    return;
		}
	    }
	    Trigger::Legato => {
		if self.other_notes_on.is_empty() {
		    return;
		}
	    }
	    _ => {}
	}
	self.note_on(note, velocity);
    }

    fn handle_note_off(&mut self, note: wmidi::Note) {
	if !self.params.key_range.covering(note) {
	    self.other_notes_on.remove(&u8::from(note));
	    return;
	}
	match self.params.trigger {
	    Trigger::Release |
	    Trigger::ReleaseKey => self.last_note_on.map_or((), |(note, velocity)| self.note_on(note,velocity)),
	    _ => {
		if !self.sustain_pedal_pushed {
		    self.note_off();
		}
	    }
	}
    }

    fn handle_control_event(&mut self, control_number: wmidi::ControlNumber, control_value: wmidi::ControlValue) {
	let (cnum, cval) = (u8::from(control_number), u8::from(control_value));

	match cnum {
	    64 => self.sustain_pedal(cval >= 64),
	    _ => {}
	}

	match self.params.on_ccs.get(&cnum) {
	    Some(cvrange) => if cvrange.covering(control_value) {
		self.note_on(self.params.pitch_keycenter, wmidi::Velocity::MAX)
	    }
	    None => {}
	}
    }

    fn pass_midi_msg(&mut self, midi_msg: &wmidi::MidiMessage) {
	match midi_msg {
	    wmidi::MidiMessage::NoteOn(_ch, note, vel) => self.handle_note_on(*note, *vel),
	    wmidi::MidiMessage::NoteOff(_ch, note, _vel) => self.handle_note_off(*note),
	    wmidi::MidiMessage::ControlChange(_ch, cnum, cval) => self.handle_control_event(*cnum, *cval),
	    _ => {}
	}
    }
}


pub struct Engine {
    pub(super) regions: Vec<Region>,
    samplerate: f64,
    max_block_length: usize
}

impl Engine {
    fn new(reg_data: Vec<RegionData>, samplerate: f64, max_block_length: usize) -> Engine {
	Engine {
	    regions: reg_data.iter().map(|rd| Region::new(rd.clone(), samplerate, max_block_length)).collect(),
	    samplerate: samplerate,
	    max_block_length: 1
	}
    }
}

impl engine::EngineTrait for Engine {
    fn midi_event(&mut self, midi_msg: &wmidi::MidiMessage) {
	for r in &mut self.regions {
	    r.pass_midi_msg(midi_msg);
	}
    }

    fn process(&mut self, out_left: &mut [f32], out_right: &mut [f32]) {
	for (l, r) in Iterator::zip(out_left.iter_mut(), out_right.iter_mut()) {
	    *l = 0.0;
	    *r = 0.0;
	}
	for r in &mut self.regions {
	    r.process(out_left, out_right);
	}
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use super::super::parser::parse_sfz_text;
    use crate::engine::EngineTrait;

    fn f32_eq(a: f32, b: f32) -> bool {
	if (a - b).abs() > f32::EPSILON {
	    println!("float equivalence check failed, a: {}, b: {}", a, b);
	    false
	} else {
	    true
	}
    }

    #[test]
    fn region_data_default() {
	let rd: RegionData = Default::default();

	assert_eq!(rd.key_range.hi, Some(wmidi::Note::HIGHEST_NOTE));
	assert_eq!(rd.key_range.lo, Some(wmidi::Note::LOWEST_NOTE));
	assert_eq!(rd.vel_range.hi, wmidi::Velocity::MAX);
	assert_eq!(rd.vel_range.lo, wmidi::Velocity::MIN);

	assert_eq!(rd.amp_veltrack, 1.0);
/* FIXME: How to test this?
	let mut env = envelopes::ADSREnvelope::new(&rd.ampeg, 1.0, 4);
	let (sustain_env, _) = env.active_envelope();
	assert_eq!(*sustain_env.as_slice(), [1.0; 4]);
*/
	assert_eq!(rd.tune, 0.0)
    }

    #[test]
    fn parse_empty_text() {
	match parse_sfz_text("".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e), "General parser error: Expecting <> tag in sfz file"),
	    _ => panic!("Expected error message")
	}
    }

    #[test]
    fn parse_sfz_hikey_lokey_region_line() {
	let regions = parse_sfz_text("<region> hikey=42 lokey=23".to_string()).unwrap();
	assert_eq!(regions.len(), 1);
	match &regions.get(0) {
	    Some(rd) => {
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::FSharp1));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::BMinus1));
		assert_eq!(rd.vel_range.hi, wmidi::Velocity::MAX);
		assert_eq!(rd.vel_range.lo, wmidi::Velocity::MIN);
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
    }

    #[test]
    fn parse_sfz_key_region_line() {
	let regions = parse_sfz_text("<region> key=42".to_string()).unwrap();
	assert_eq!(regions.len(), 1);
	match &regions.get(0) {
	    Some(rd) => {
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::FSharp1));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::FSharp1));
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
    }

    #[test]
    fn parse_sfz_hikey_lokey_notefmt_region_line() {
	let regions = parse_sfz_text("<region> hikey=c#3 lokey=ab2 <region> hikey=c3 lokey=a2".to_string()).unwrap();
	assert_eq!(regions.len(), 2);
	match &regions.get(0) {
	    Some(rd) => {
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::Db2));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::GSharp1));
		assert_eq!(rd.vel_range.hi, wmidi::Velocity::MAX);
		assert_eq!(rd.vel_range.lo, wmidi::Velocity::MIN);
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
	match &regions.get(1) {
	    Some(rd) => {
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::C2));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::A1));
		assert_eq!(rd.vel_range.hi, wmidi::Velocity::MAX);
		assert_eq!(rd.vel_range.lo, wmidi::Velocity::MIN);
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
    }

    #[test]
    fn parse_sfz_hikey_lokey_group_line() {
	let regions = parse_sfz_text("<group> hivel=42 lovel=23".to_string()).unwrap();
	assert_eq!(regions.len(), 0);
    }

    #[test]
    fn parse_sfz_invalid_header_line() {
	match parse_sfz_text("<foo> hikey=42 lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e), "Unknown key: foo"),
	    _ => panic!("Not seen expected error")
	}
    }

    #[test]
    fn parse_sfz_invalid_opcode_line() {
	match parse_sfz_text("<region> foo=42 lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e), "Unknown key: foo"),
	    _ => panic!("Not seen expected error")
	}
    }

    #[test]
    fn parse_sfz_invalid_non_int_value_line() {
	match parse_sfz_text("<region> hikey=aa lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e), "Invalid key: aa"),
	    _ => panic!("Not seen expected error")
	}
    }

    /* FIXME: How to test this?
    #[test]
    fn parse_ampeg() {
	let regions = parse_sfz_text("<region> ampeg_attack=23 ampeg_hold=42 ampeg_decay=47 ampeg_sustain=11 ampeg_release=0.2342".to_string()).unwrap();
	match regions.get(0) {
	    Some(rd) => {
		assert_eq!(rd.ampeg.attack, 23.0);
		assert_eq!(rd.ampeg.hold, 42.0);
		assert_eq!(rd.ampeg.decay, 47.0);
		assert_eq!(rd.ampeg.sustain, 0.11);
		assert_eq!(rd.ampeg.release, 0.2342);
	    }
	    None => panic!("expeted region with ampeg")
	}
    }
     */

    #[test]
    fn parse_out_of_range_amp_veltrack() {
	match parse_sfz_text("<region> amp_veltrack=105 lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e), "amp_veltrack out of range: -100 <= 105 <= 100"),
	    _ => panic!("Not seen expected error")
	}
	match parse_sfz_text("<region> amp_veltrack=-105 lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e), "amp_veltrack out of range: -100 <= -105 <= 100"),
	    _ => panic!("Not seen expected error")
	}
    }

    #[test]
    fn parse_out_of_range_ampeg_attack() {
	match parse_sfz_text("<region> ampeg_attack=105 lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e), "ampeg_attack out of range: 0 <= 105 <= 100"),
	    _ => panic!("Not seen expected error")
	}
	match parse_sfz_text("<region> ampeg_attack=-20 lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e), "ampeg_attack out of range: 0 <= -20 <= 100"),
	    _ => panic!("Not seen expected error")
	}
	match parse_sfz_text("<region> ampeg_attack=aa lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e),  "invalid float literal"),
	    _ => panic!("Not seen expected error")
	}
    }

        #[test]
    fn parse_out_of_range_ampeg_hold() {
	match parse_sfz_text("<region> ampeg_hold=105 lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e), "ampeg_hold out of range: 0 <= 105 <= 100"),
	    _ => panic!("Not seen expected error")
	}
	match parse_sfz_text("<region> ampeg_hold=-20 lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e), "ampeg_hold out of range: 0 <= -20 <= 100"),
	    _ => panic!("Not seen expected error")
	}
	match parse_sfz_text("<region> ampeg_hold=aa lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e),  "invalid float literal"),
	    _ => panic!("Not seen expected error")
	}
    }

    #[test]
    fn parse_out_of_range_ampeg_decay() {
	match parse_sfz_text("<region> ampeg_decay=105 lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e), "ampeg_decay out of range: 0 <= 105 <= 100"),
	    _ => panic!("Not seen expected error")
	}
	match parse_sfz_text("<region> ampeg_decay=-20 lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e), "ampeg_decay out of range: 0 <= -20 <= 100"),
	    _ => panic!("Not seen expected error")
	}
	match parse_sfz_text("<region> ampeg_decay=aa lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e),  "invalid float literal"),
	    _ => panic!("Not seen expected error")
	}
    }

    #[test]
    fn parse_out_of_range_ampeg_sustain() {
	match parse_sfz_text("<region> ampeg_sustain=105 lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e), "ampeg_sustain out of range: 0 <= 105 <= 100"),
	    _ => panic!("Not seen expected error")
	}
	match parse_sfz_text("<region> ampeg_sustain=-20 lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e), "ampeg_sustain out of range: 0 <= -20 <= 100"),
	    _ => panic!("Not seen expected error")
	}
	match parse_sfz_text("<region> ampeg_sustain=aa lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e),  "invalid float literal"),
	    _ => panic!("Not seen expected error")
	}
    }

    #[test]
    fn parse_out_of_range_ampeg_release() {
	match parse_sfz_text("<region> ampeg_release=105 lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e), "ampeg_release out of range: 0 <= 105 <= 100"),
	    _ => panic!("Not seen expected error")
	}
	match parse_sfz_text("<region> ampeg_release=-20 lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e), "ampeg_release out of range: 0 <= -20 <= 100"),
	    _ => panic!("Not seen expected error")
	}
	match parse_sfz_text("<region> ampeg_release=aa lokey=23".to_string()) {
	    Err(e) => assert_eq!(format!("{}", e),  "invalid float literal"),
	    _ => panic!("Not seen expected error")
	}
    }

    #[test]
    fn parse_sfz_comment_in_line() {
	let regions = parse_sfz_text("<region> hivel=42 lovel=23 // foo".to_string()).unwrap();
	assert_eq!(regions.len(), 1);
	match &regions.get(0) {
	    Some(rd) => {
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::HIGHEST_NOTE));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::LOWEST_NOTE));
		assert_eq!(u8::from(rd.vel_range.hi), 42);
		assert_eq!(u8::from(rd.vel_range.lo), 23);
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
    }

    #[test]
    fn parse_region_line_span() {
	let regions = parse_sfz_text("<region> hivel=42 lovel=23 \n hikey=43 lokey=24".to_string()).unwrap();
	assert_eq!(regions.len(), 1);
	match &regions.get(0) {
	    Some(rd) => {
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::G1));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::C0));
		assert_eq!(u8::from(rd.vel_range.hi), 42);
		assert_eq!(u8::from(rd.vel_range.lo), 23);
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
    }

    #[test]
    fn parse_region_line_span_with_coment() {
	let regions = parse_sfz_text("<region> hivel=42 lovel=23 // foo bar foo\nhikey=43 lokey=24".to_string()).unwrap();
	assert_eq!(regions.len(), 1);
	match &regions.get(0) {
	    Some(rd) => {
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::G1));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::C0));
		assert_eq!(u8::from(rd.vel_range.hi), 42);
		assert_eq!(u8::from(rd.vel_range.lo), 23);
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
    }

    #[test]
    fn parse_two_region_line() {
	let s = "<region> hivel=41 lovel=22 <region> hikey=42 lokey=23";

	let regions = parse_sfz_text(s.to_string()).unwrap();
	assert_eq!(regions.len(), 2)
    }

    #[test]
    fn parse_regions_inheriting_group_data() {
	let s = "
<group> hivel=42
<region> lovel=23
<region> lovel=21
";
	let regions = parse_sfz_text(s.to_string()).unwrap();
	assert_eq!(regions.len(), 2);
	match &regions.get(0) {
	    Some(rd) => {
		assert_eq!(u8::from(rd.vel_range.hi), 42);
		assert_eq!(u8::from(rd.vel_range.lo), 23)
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
	match &regions.get(1) {
	    Some(rd) => {
		assert_eq!(u8::from(rd.vel_range.hi), 42);
		assert_eq!(u8::from(rd.vel_range.lo), 21)
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
    }

    #[test]
    fn parse_regions_inheriting_group_data_2groups() {
	let s = "
<group> hivel=42 hikey=41
<region> lokey=23
<region> lovel=21
<group> hikey=42 hivel=41
<region> lokey=23
<region> lovel=21
<region> hikey=43 hivel=42 lokey=23
<region> lovel=23
";
	let regions = parse_sfz_text(s.to_string()).unwrap();
	assert_eq!(regions.len(), 6);
	match &regions.get(0) {
	    Some(rd) => {
		assert_eq!(u8::from(rd.vel_range.hi), 42);
		assert_eq!(rd.vel_range.lo, wmidi::Velocity::MIN);
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::F1));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::BMinus1));
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
	match &regions.get(1) {
	    Some(rd) => {
		assert_eq!(u8::from(rd.vel_range.hi), 42);
		assert_eq!(u8::from(rd.vel_range.lo), 21);
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::F1));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::LOWEST_NOTE));
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
	match &regions.get(2) {
	    Some(rd) => {
		assert_eq!(u8::from(rd.vel_range.hi), 41);
		assert_eq!(rd.vel_range.lo, wmidi::Velocity::MIN);
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::FSharp1));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::BMinus1));
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
	match &regions.get(3) {
	    Some(rd) => {
		assert_eq!(u8::from(rd.vel_range.hi), 41);
		assert_eq!(u8::from(rd.vel_range.lo), 21);
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::FSharp1));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::LOWEST_NOTE));
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
	match &regions.get(4) {
	    Some(rd) => {
		assert_eq!(u8::from(rd.vel_range.hi), 42);
		assert_eq!(rd.vel_range.lo, wmidi::Velocity::MIN);
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::G1));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::BMinus1));
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
	match &regions.get(5) {
	    Some(rd) => {
		assert_eq!(u8::from(rd.vel_range.hi), 41);
		assert_eq!(u8::from(rd.vel_range.lo), 23);
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::FSharp1));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::LOWEST_NOTE));
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
    }

    #[test]
    fn parse_shortened_real_life_sfz() {
	let s = r#"
//=====================================
// Salamander Grand Piano V2
// (only a small part for testing the parser)
// Author: Alexander Holm
// Contact: axeldenstore [at] gmail [dot] com
// License: CC-by
//
//=====================================

//Notes
<group> amp_veltrack=73 ampeg_release=1

<region> sample=48khz24bit\A0v1.wav lokey=21 hikey=22 lovel=1 hivel=26 pitch_keycenter=21 tune=10
<region> sample=48khz24bit\A0v2.wav lokey=21 hikey=22 lovel=27 hivel=34 pitch_keycenter=21 tune=10

//========================
//Notes without dampers
<group> amp_veltrack=73 ampeg_release=5

<region> sample=48khz24bit\F#6v1.wav lokey=89 hikey=91 lovel=1 hivel=26 pitch_keycenter=90 tune=-13
<region> sample=48khz24bit\F#6v2.wav lokey=89 hikey=91 lovel=27 hivel=34 pitch_keycenter=90 tune=-13
//Release string resonances
<group> trigger=release volume=-4 amp_veltrack=94 rt_decay=6

<region> sample=48khz24bit\harmLA0.wav lokey=20 hikey=22 lovel=45 pitch_keycenter=21
<region> sample=48khz24bit\harmLC1.wav lokey=23 hikey=25 lovel=45 pitch_keycenter=24

//======================
//HammerNoise
<group> trigger=release pitch_keytrack=0 volume=-37 amp_veltrack=82 rt_decay=2

<region> sample=48khz24bit\rel1.wav lokey=21 hikey=21
<region> sample=48khz24bit\rel2.wav lokey=22 hikey=22
//======================
//pedalAction

<group> group=1 hikey=-1 lokey=-1 on_locc64=126 on_hicc64=127 off_by=2 volume=-20

<region> sample=48khz24bit\pedalD1.wav lorand=0 hirand=0.5
<region> sample=48khz24bit\pedalD2.wav lorand=0.5 hirand=1

<group> group=2 hikey=-1 lokey=-1 on_locc64=0 on_hicc64=1 volume=-19

<region> sample=48khz24bit\pedalU1.wav lorand=0 hirand=0.5
<region> sample=48khz24bit\pedalU2.wav lorand=0.5 hirand=1

"#;
	let regions = parse_sfz_text(s.to_string()).unwrap();

	assert_eq!(regions.len(), 12);
	match &regions.get(0) {
	    Some(rd) => {
		assert_eq!(rd.amp_veltrack, 0.73);
		// FIXME: how to test this? assert_eq!(rd.ampeg.release, 1.0);
		assert_eq!(rd.pitch_keycenter, wmidi::Note::AMinus1);
		assert_eq!(rd.tune, 0.1);
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::BbMinus1));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::AMinus1));
		assert_eq!(u8::from(rd.vel_range.hi), 26);
		assert_eq!(u8::from(rd.vel_range.lo), 1);
		assert_eq!(rd.sample, "48khz24bit\\A0v1.wav");
		assert_eq!(rd.trigger, Trigger::Attack);
		assert_eq!(rd.rt_decay, 0.0);
		assert_eq!(rd.pitch_keytrack, 1.0);
		assert_eq!(rd.group, 0);
		assert_eq!(rd.off_by, 0);
		assert!(rd.on_ccs.is_empty(), (0, 0));
		assert_eq!(rd.random_range.hi, 0.0);
		assert_eq!(rd.random_range.lo, 0.0);
		assert_eq!(rd.volume, 0.0);
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
	match &regions.get(1) {
	    Some(rd) => {
		assert_eq!(rd.amp_veltrack, 0.73);
		// FIXME: how to test this? assert_eq!(rd.ampeg.release, 1.0);
		assert_eq!(rd.pitch_keycenter, wmidi::Note::AMinus1);
		assert_eq!(rd.tune, 0.1);
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::BbMinus1));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::AMinus1));
		assert_eq!(u8::from(rd.vel_range.hi), 34);
		assert_eq!(u8::from(rd.vel_range.lo), 27);
		assert_eq!(rd.sample, "48khz24bit\\A0v2.wav");
		assert_eq!(rd.trigger, Trigger::Attack);
		assert_eq!(rd.rt_decay, 0.0);
		assert_eq!(rd.pitch_keytrack, 1.0);
		assert_eq!(rd.group, 0);
		assert_eq!(rd.off_by, 0);
		assert!(rd.on_ccs.is_empty(), (0, 0));
		assert_eq!(rd.random_range.hi, 0.0);
		assert_eq!(rd.random_range.lo, 0.0);
		assert_eq!(rd.volume, 0.0);
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
	match &regions.get(2) {
	    Some(rd) => {
		assert_eq!(rd.amp_veltrack, 0.73);
		// FIXME: how to test this? assert_eq!(rd.ampeg.release, 5.0);
		assert_eq!(rd.pitch_keycenter, wmidi::Note::Gb5);
		assert_eq!(rd.tune, -0.13);
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::G5));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::F5));
		assert_eq!(u8::from(rd.vel_range.hi), 26);
		assert_eq!(u8::from(rd.vel_range.lo), 1);
		assert_eq!(rd.sample, "48khz24bit\\F#6v1.wav");
		assert_eq!(rd.trigger, Trigger::Attack);
		assert_eq!(rd.rt_decay, 0.0);
		assert_eq!(rd.pitch_keytrack, 1.0);
		assert_eq!(rd.group, 0);
		assert_eq!(rd.off_by, 0);
		assert!(rd.on_ccs.is_empty(), (0, 0));
		assert_eq!(rd.random_range.hi, 0.0);
		assert_eq!(rd.random_range.lo, 0.0);
		assert_eq!(rd.volume, 0.0);
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
	match &regions.get(3) {
	    Some(rd) => {
		assert_eq!(rd.amp_veltrack, 0.73);
		// FIXME: how to test this? assert_eq!(rd.ampeg.release, 5.0);
		assert_eq!(rd.pitch_keycenter, wmidi::Note::Gb5);
		assert_eq!(rd.tune, -0.13);
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::G5));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::F5));
		assert_eq!(u8::from(rd.vel_range.hi), 34);
		assert_eq!(u8::from(rd.vel_range.lo), 27);
		assert_eq!(rd.sample, "48khz24bit\\F#6v2.wav");
		assert_eq!(rd.trigger, Trigger::Attack);
		assert_eq!(rd.rt_decay, 0.0);
		assert_eq!(rd.pitch_keytrack, 1.0);
		assert_eq!(rd.group, 0);
		assert_eq!(rd.off_by, 0);
		assert!(rd.on_ccs.is_empty(), (0, 0));
		assert_eq!(rd.random_range.hi, 0.0);
		assert_eq!(rd.random_range.lo, 0.0);
		assert_eq!(rd.volume, 0.0);
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
	match &regions.get(4) {
	    Some(rd) => {
		assert_eq!(rd.amp_veltrack, 0.94);
		// FIXME: how to test this? assert_eq!(rd.ampeg.release, 0.0);
		assert_eq!(rd.pitch_keycenter, wmidi::Note::AMinus1);
		assert_eq!(rd.tune, 0.0);
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::BbMinus1));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::AbMinus1));
		assert_eq!(rd.vel_range.hi, wmidi::Velocity::MAX);
		assert_eq!(u8::from(rd.vel_range.lo), 45);
		assert_eq!(rd.sample, "48khz24bit\\harmLA0.wav");
		assert_eq!(rd.trigger, Trigger::Release);
		assert_eq!(rd.rt_decay, 6.0);
		assert_eq!(rd.pitch_keytrack, 1.0);
		assert_eq!(rd.group, 0);
		assert_eq!(rd.off_by, 0);
		assert!(rd.on_ccs.is_empty(), (0, 0));
		assert_eq!(rd.random_range.hi, 0.0);
		assert_eq!(rd.random_range.lo, 0.0);
		assert_eq!(rd.volume, -4.0);
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
	match &regions.get(5) {
	    Some(rd) => {
		assert_eq!(rd.amp_veltrack, 0.94);
		// FIXME: how to test this? assert_eq!(rd.ampeg.release, 0.0);
		assert_eq!(rd.pitch_keycenter, wmidi::Note::C0);
		assert_eq!(rd.tune, 0.0);
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::Db0));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::BMinus1));
		assert_eq!(rd.vel_range.hi, wmidi::Velocity::MAX);
		assert_eq!(u8::from(rd.vel_range.lo), 45);
		assert_eq!(rd.sample, "48khz24bit\\harmLC1.wav");
		assert_eq!(rd.trigger, Trigger::Release);
		assert_eq!(rd.rt_decay, 6.0);
		assert_eq!(rd.pitch_keytrack, 1.0);
		assert_eq!(rd.group, 0);
		assert_eq!(rd.off_by, 0);
		assert!(rd.on_ccs.is_empty(), (0, 0));
		assert_eq!(rd.random_range.hi, 0.0);
		assert_eq!(rd.random_range.lo, 0.0);
		assert_eq!(rd.volume, -4.0);
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
	match &regions.get(6) {
	    Some(rd) => {
		assert_eq!(rd.amp_veltrack, 0.82);
		// FIXME: how to test this? assert_eq!(rd.ampeg.release, 0.0);
		assert_eq!(rd.pitch_keycenter, wmidi::Note::C3);
		assert_eq!(rd.tune, 0.0);
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::AMinus1));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::AMinus1));
		assert_eq!(rd.vel_range.hi, wmidi::Velocity::MAX);
		assert_eq!(rd.vel_range.lo, wmidi::Velocity::MIN);
		assert_eq!(rd.sample, "48khz24bit\\rel1.wav");
		assert_eq!(rd.trigger, Trigger::Release);
		assert_eq!(rd.rt_decay, 2.0);
		assert_eq!(rd.pitch_keytrack, 0.0);
		assert_eq!(rd.group, 0);
		assert_eq!(rd.off_by, 0);
		assert!(rd.on_ccs.is_empty(), (0, 0));
		assert_eq!(rd.random_range.hi, 0.0);
		assert_eq!(rd.random_range.lo, 0.0);
		assert_eq!(rd.volume, -37.0);
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
	match &regions.get(7) {
	    Some(rd) => {
		assert_eq!(rd.amp_veltrack, 0.82);
		// FIXME: how to test this? assert_eq!(rd.ampeg.release, 0.0);
		assert_eq!(rd.pitch_keycenter, wmidi::Note::C3);
		assert_eq!(rd.tune, 0.0);
		assert_eq!(rd.key_range.hi, Some(wmidi::Note::ASharpMinus1));
		assert_eq!(rd.key_range.lo, Some(wmidi::Note::ASharpMinus1));
		assert_eq!(rd.vel_range.hi, wmidi::Velocity::MAX);
		assert_eq!(rd.vel_range.lo, wmidi::Velocity::MIN);
		assert_eq!(rd.sample, "48khz24bit\\rel2.wav");
		assert_eq!(rd.trigger, Trigger::Release);
		assert_eq!(rd.rt_decay, 2.0);
		assert_eq!(rd.pitch_keytrack, 0.0);
		assert_eq!(rd.group, 0);
		assert_eq!(rd.off_by, 0);
		assert!(rd.on_ccs.is_empty(), (0, 0));
		assert_eq!(rd.random_range.hi, 0.0);
		assert_eq!(rd.random_range.lo, 0.0);
		assert_eq!(rd.volume, -37.0);
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
	match &regions.get(8) {
	    Some(rd) => {
		assert_eq!(rd.amp_veltrack, 1.0);
		// FIXME: how to test this? assert_eq!(rd.ampeg.release, 0.0);
		assert_eq!(rd.pitch_keycenter, wmidi::Note::C3);
		assert_eq!(rd.tune, 0.0);
		assert_eq!(rd.key_range.hi, None);
		assert_eq!(rd.key_range.lo, None);
		assert_eq!(rd.vel_range.hi, wmidi::Velocity::MAX);
		assert_eq!(rd.vel_range.lo, wmidi::Velocity::MIN);
		assert_eq!(rd.sample, "48khz24bit\\pedalD1.wav");
		assert_eq!(rd.trigger, Trigger::Attack);
		assert_eq!(rd.rt_decay, 0.0);
		assert_eq!(rd.pitch_keytrack, 1.0);
		assert_eq!(rd.group, 1);
		assert_eq!(rd.off_by, 2);
		assert!(rd.on_ccs.get(&64).unwrap().covering(wmidi::ControlValue::try_from(126).unwrap()));
		assert_eq!(rd.random_range.hi, 0.5);
		assert_eq!(rd.random_range.lo, 0.0);
		assert_eq!(rd.volume, -20.0);
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
	match &regions.get(9) {
	    Some(rd) => {
		assert_eq!(rd.amp_veltrack, 1.0);
		// FIXME: how to test this? assert_eq!(rd.ampeg.release, 0.0);
		assert_eq!(rd.pitch_keycenter, wmidi::Note::C3);
		assert_eq!(rd.tune, 0.0);
		assert_eq!(rd.key_range.hi, None);
		assert_eq!(rd.key_range.lo, None);
		assert_eq!(rd.vel_range.hi, wmidi::Velocity::MAX);
		assert_eq!(rd.vel_range.lo, wmidi::Velocity::MIN);
		assert_eq!(rd.sample, "48khz24bit\\pedalD2.wav");
		assert_eq!(rd.trigger, Trigger::Attack);
		assert_eq!(rd.rt_decay, 0.0);
		assert_eq!(rd.pitch_keytrack, 1.0);
		assert_eq!(rd.group, 1);
		assert_eq!(rd.off_by, 2);
		assert!(rd.on_ccs.get(&64).unwrap().covering(wmidi::ControlValue::try_from(127).unwrap()));
		assert_eq!(rd.random_range.hi, 1.0);
		assert_eq!(rd.random_range.lo, 0.5);
		assert_eq!(rd.volume, -20.0);
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
	match &regions.get(10) {
	    Some(rd) => {
		assert_eq!(rd.amp_veltrack, 1.0);
		// FIXME: how to test this? assert_eq!(rd.ampeg.release, 0.0);
		assert_eq!(rd.pitch_keycenter, wmidi::Note::C3);
		assert_eq!(rd.tune, 0.0);
		assert_eq!(rd.key_range.hi, None);
		assert_eq!(rd.key_range.lo, None);
		assert_eq!(rd.vel_range.hi, wmidi::Velocity::MAX);
		assert_eq!(rd.vel_range.lo, wmidi::Velocity::MIN);
		assert_eq!(rd.sample, "48khz24bit\\pedalU1.wav");
		assert_eq!(rd.trigger, Trigger::Attack);
		assert_eq!(rd.rt_decay, 0.0);
		assert_eq!(rd.pitch_keytrack, 1.0);
		assert_eq!(rd.group, 2);
		assert_eq!(rd.off_by, 0);
		assert!(rd.on_ccs.get(&64).unwrap().covering(wmidi::ControlValue::try_from(1).unwrap()));
		assert_eq!(rd.random_range.hi, 0.5);
		assert_eq!(rd.random_range.lo, 0.0);
		assert_eq!(rd.volume, -19.0);
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
	match &regions.get(11) {
	    Some(rd) => {
		assert_eq!(rd.amp_veltrack, 1.0);
		// FIXME: how to test this? assert_eq!(rd.ampeg.release, 0.0);
		assert_eq!(rd.pitch_keycenter, wmidi::Note::C3);
		assert_eq!(rd.tune, 0.0);
		assert_eq!(rd.key_range.hi, None);
		assert_eq!(rd.key_range.lo, None);
		assert_eq!(rd.vel_range.hi, wmidi::Velocity::MAX);
		assert_eq!(rd.vel_range.lo, wmidi::Velocity::MIN);
		assert_eq!(rd.sample, "48khz24bit\\pedalU2.wav");
		assert_eq!(rd.trigger, Trigger::Attack);
		assert_eq!(rd.rt_decay, 0.0);
		assert_eq!(rd.pitch_keytrack, 1.0);
		assert_eq!(rd.group, 2);
		assert_eq!(rd.off_by, 0);
		assert!(rd.on_ccs.get(&64).unwrap().covering(wmidi::ControlValue::try_from(0).unwrap()));
		assert_eq!(rd.random_range.hi, 1.0);
		assert_eq!(rd.random_range.lo, 0.5);
		assert_eq!(rd.volume, -19.0);
	    }
	    _ => panic!("Expected region, got somthing different.")
	}
    }

    /*
    #[test]
    fn generate_adsr_envelope() {
	let regions = parse_sfz_text("<region> ampeg_attack=2 ampeg_hold=3 ampeg_decay=4 ampeg_sustain=60 ampeg_release=5".to_string()).unwrap();
	let region = regions.get(0).unwrap();

	let ads: Vec<f32> = region.ampeg.ads_envelope(1.0, 12)[..12].iter().map(|v| (v*100.0).round()/100.0).collect();
	assert_eq!(ads.as_slice(), [0.0, 0.5, 1.0, 1.0, 1.0, 0.65, 0.61, 0.6, 0.6, 0.6, 0.6, 0.6]);

	let rel: Vec<f32> = region.ampeg.release_envelope(1.0, 8).iter().map(|v| (v*10000.0).round()/10000.0).collect();
	assert_eq!(rel.as_slice(), [0.1211, 0.0245, 0.0049, 0.0010, 0.0002, 0.0, 0.0, 0.0]);
    }
    */

    #[test]
    fn simple_region_process() {
	let sample = vec![1.0, 0.5,
			  0.5, 1.0,
			  1.0, 0.5];

	let mut region = Region::new(RegionData::default(), 1.0, 8);
	region.set_sample_data(sample);

	region.note_on(wmidi::Note::C3, wmidi::Velocity::MAX);

	let mut out_left: [f32; 2] = [0.0, 0.0];
	let mut out_right: [f32; 2] = [0.0, 0.0];

	region.process(&mut out_left, &mut out_right);
	assert!(f32_eq(out_left[0], 1.0));
	assert!(f32_eq(out_left[1], 0.5));

	assert!(f32_eq(out_right[0], 0.5));
	assert!(f32_eq(out_right[1], 1.0));

	assert!(region.is_active());

	let mut out_left: [f32; 2] = [-0.5, -0.2];
	let mut out_right: [f32; 2] = [-0.2, -0.5];

	region.process(&mut out_left, &mut out_right);
	assert!(f32_eq(out_left[0], 0.5));
	assert!(f32_eq(out_left[1], -0.2));

	assert!(f32_eq(out_right[0], 0.3));
	assert!(f32_eq(out_right[1], -0.5));

	assert!(!region.is_active());
    }

    #[test]
    fn region_volume_process() {
	let sample = vec![1.0, 1.0];

	let mut region_data = RegionData::default();
	region_data.set_volume(-20.0).unwrap();

	let mut region = Region::new(region_data, 1.0, 8);
	region.set_sample_data(sample.clone());

	region.note_on(wmidi::Note::C3, wmidi::Velocity::MAX);

	let mut out_left: [f32; 2] = [0.0, 0.0];
	let mut out_right: [f32; 2] = [0.0, 0.0];

	region.process(&mut out_left, &mut out_right);

	assert_eq!(out_left[0], 0.1);
	assert_eq!(out_right[0], 0.1);
    }

    #[test]
    fn region_amp_envelope_process() {
	let mut sample = vec![];
	sample.resize(32, 1.0);
	let regions = parse_sfz_text("<region> ampeg_attack=2 ampeg_hold=3 ampeg_decay=4 ampeg_sustain=60 ampeg_release=5".to_string()).unwrap();

	let mut region = Region::new(regions.get(0).unwrap().clone(), 1.0, 16);
	region.set_sample_data(sample.clone());
	region.note_on(wmidi::Note::C3, wmidi::Velocity::MAX);

	let mut out_left: [f32; 12] = [0.0; 12];
	let mut out_right: [f32; 12] = [0.0; 12];

	region.process(&mut out_left, &mut out_right);

	let out: Vec<f32> = out_left.iter().map(|v| (v*100.0).round()/100.0).collect();
	assert_eq!(out.as_slice(), [0.0, 0.5, 1.0, 1.0, 1.0, 0.65, 0.61, 0.6, 0.6, 0.6, 0.6, 0.6]);
    }

  #[test]
    fn region_amp_envelope_process_sustain() {
	let sample = vec![1.0; 96];

	let regions = parse_sfz_text("<region> ampeg_attack=2 ampeg_hold=3 ampeg_decay=4 ampeg_sustain=60 ampeg_release=5".to_string()).unwrap();

	let mut region = Region::new(regions.get(0).unwrap().clone(), 1.0, 12);
	region.set_sample_data(sample.clone());
	region.note_on(wmidi::Note::C3, wmidi::Velocity::MAX);

	let mut out_left: [f32; 12] = [0.0; 12];
	let mut out_right: [f32; 12] = [0.0; 12];

	region.process(&mut out_left, &mut out_right);

	let out: Vec<f32> = out_left.iter().map(|v| (v*100.0).round()/100.0).collect();
	assert_eq!(out.as_slice(), [0.0, 0.5, 1.0, 1.0, 1.0, 0.65, 0.61, 0.6, 0.6, 0.6, 0.6, 0.6]);

	let mut out_left: [f32; 12] = [0.0; 12];
	let mut out_right: [f32; 12] = [0.0; 12];

	region.process(&mut out_left, &mut out_right);
	let out: Vec<f32> = out_left.iter().map(|v| (v*1000.0).round()/1000.0).collect();
	assert_eq!(out, [0.6; 12]);

	let mut out_left: [f32; 12] = [0.0; 12];
	let mut out_right: [f32; 12] = [0.0; 12];

	region.process(&mut out_left, &mut out_right);
	let out: Vec<f32> = out_left.iter().map(|v| (v*1000.0).round()/1000.0).collect();
	assert_eq!(out, [0.6; 12]);

    	let mut out_left: [f32; 12] = [0.0; 12];
	let mut out_right: [f32; 12] = [0.0; 12];

	region.process(&mut out_left, &mut out_right);
	let out: Vec<f32> = out_left.iter().map(|v| (v*1000.0).round()/1000.0).collect();
	assert_eq!(out, [0.6; 12]);
    }


    #[test]
    fn engine_process_silence() {
	let mut engine = Engine::new(vec![RegionData::default(), RegionData::default()], 1.0, 16);

	let mut out_left: [f32; 4] = [1.0; 4];
	let mut out_right: [f32; 4] = [1.0; 4];

	engine.process(&mut out_left, &mut out_right);

	assert_eq!(out_left, [0.0; 4]);
	assert_eq!(out_right, [0.0; 4]);
    }


    #[test]
    fn simple_engine_process() {
	let sample1 = vec![1.0, 0.5,
			   0.5, 1.0,
			   1.0, 0.5];
	let sample2 = vec![-0.5, 0.5,
			   -0.5, -0.5,
			   0.0, 0.5];

	let mut engine = Engine::new(vec![RegionData::default(), RegionData::default()], 1.0, 16);

	engine.regions[0].set_sample_data(sample1);
	engine.regions[0].note_on(wmidi::Note::C3, wmidi::Velocity::MAX);
	engine.regions[1].set_sample_data(sample2);
	engine.regions[1].note_on(wmidi::Note::C3, wmidi::Velocity::MAX);

	let mut out_left: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
	let mut out_right: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

	engine.process(&mut out_left, &mut out_right);

	assert!(!engine.regions[0].is_active());
	assert!(!engine.regions[1].is_active());

	assert_eq!(out_left[0], 0.5);
	assert_eq!(out_left[1], 0.0);
	assert_eq!(out_left[2], 1.0);

	assert_eq!(out_right[0], 1.0);
	assert_eq!(out_right[1], 0.5);
	assert_eq!(out_right[2], 1.0);
    }

    #[test]
    fn note_trigger_key_range() {
	let mut rd = RegionData::default();
	rd.key_range.set_hi(70).unwrap();
	rd.key_range.set_lo(60).unwrap();
	let mut region = Region::new(rd, 1.0, 2);

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::E2, wmidi::Velocity::MAX));
	assert!(!region.is_active());

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::E2, wmidi::Velocity::MIN));
	assert!(!region.is_active());

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::E3, wmidi::Velocity::try_from(63).unwrap()));
	assert!(region.is_active());
	assert_eq!(region.gain, 0.24607849215698431397);

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::E3, wmidi::Velocity::MIN));
	assert!(!region.is_active());
    }


    #[test]
    fn note_trigger_vel_range() {
	let mut rd = RegionData::default();
	rd.vel_range.set_hi(70).unwrap();
	rd.vel_range.set_lo(60).unwrap();
	let mut region = Region::new(rd, 1.0, 2);

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::try_from(90).unwrap()));
	assert!(!region.is_active());

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MIN));
	assert!(!region.is_active());


	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::try_from(63).unwrap()));
	assert!(region.is_active());
	assert_eq!(region.gain, 0.24607849215698431397);

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MIN));
	assert!(!region.is_active());
    }

    #[test]
    fn region_trigger_cc() {
	let mut rd = RegionData::default();
	rd.push_on_lo_cc(64, 63).unwrap();
	rd.push_on_hi_cc(64, 127).unwrap();
	rd.push_on_hi_cc(42, 23).unwrap();

	let mut region = Region::new(rd, 1.0, 2);

	region.pass_midi_msg(&wmidi::MidiMessage::ControlChange(wmidi::Channel::Ch1,
								wmidi::ControlNumber::try_from(23).unwrap(),
								wmidi::ControlValue::try_from(90).unwrap()));
	assert!(!region.is_active());

	region.pass_midi_msg(&wmidi::MidiMessage::ControlChange(wmidi::Channel::Ch1,
								wmidi::ControlNumber::try_from(64).unwrap(),
								wmidi::ControlValue::try_from(23).unwrap()));
	assert!(!region.is_active());

	region.pass_midi_msg(&wmidi::MidiMessage::ControlChange(wmidi::Channel::Ch1,
								wmidi::ControlNumber::try_from(42).unwrap(),
								wmidi::ControlValue::try_from(21).unwrap()));
	assert!(!region.is_active());

	region.pass_midi_msg(&wmidi::MidiMessage::ControlChange(wmidi::Channel::Ch1,
								wmidi::ControlNumber::try_from(64).unwrap(),
								wmidi::ControlValue::try_from(90).unwrap()));
	assert!(region.is_active());

    }


    #[test]
    fn note_trigger_release() {
	let mut rd = RegionData::default();
	rd.set_trigger(Trigger::Release);
	let mut region = Region::new(rd, 1.0, 2);

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::try_from(63).unwrap()));
	assert!(!region.is_active());

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MAX));
	assert!(region.is_active());
	assert_eq!(region.gain, 0.24607849215698431397);
    }

    #[test]
    fn note_trigger_release_sustain_pedal() {
    	let mut rd = RegionData::default();
	rd.set_trigger(Trigger::Release);
	let mut region = Region::new(rd, 1.0, 2);

	// sustain pedal on
	region.pass_midi_msg(&wmidi::MidiMessage::ControlChange(
	    wmidi::Channel::Ch1,
	    unsafe { wmidi::ControlNumber::from_unchecked(64) },
	    unsafe { wmidi::ControlValue::from_unchecked(64) }
	));

	// sustain pedal off
	region.pass_midi_msg(&wmidi::MidiMessage::ControlChange(
	    wmidi::Channel::Ch1,
	    unsafe { wmidi::ControlNumber::from_unchecked(64) },
	    unsafe { wmidi::ControlValue::from_unchecked(63) }
	));

	assert!(!region.is_active());

	// sustain pedal on
	region.pass_midi_msg(&wmidi::MidiMessage::ControlChange(
	    wmidi::Channel::Ch1,
	    unsafe { wmidi::ControlNumber::from_unchecked(64) },
	    unsafe { wmidi::ControlValue::from_unchecked(64) }
	));

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, unsafe { wmidi::Velocity::from_unchecked(63) }));
	assert!(!region.is_active());

	// sustain pedal off
	region.pass_midi_msg(&wmidi::MidiMessage::ControlChange(
	    wmidi::Channel::Ch1,
	    unsafe { wmidi::ControlNumber::from_unchecked(64) },
	    unsafe { wmidi::ControlValue::from_unchecked(63) }
	));

	assert!(region.is_active());
	assert_eq!(region.gain, 0.24607849215698431397);


	let mut rd = RegionData::default();
	rd.set_trigger(Trigger::Release);
	let mut region = Region::new(rd, 1.0, 2);

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, unsafe { wmidi::Velocity::from_unchecked(63) }));
	assert!(!region.is_active());

    	// sustain pedal on
	region.pass_midi_msg(&wmidi::MidiMessage::ControlChange(
	    wmidi::Channel::Ch1,
	    unsafe { wmidi::ControlNumber::from_unchecked(64) },
	    unsafe { wmidi::ControlValue::from_unchecked(64) }
	));

	// sustain pedal off
	region.pass_midi_msg(&wmidi::MidiMessage::ControlChange(
	    wmidi::Channel::Ch1,
	    unsafe { wmidi::ControlNumber::from_unchecked(64) },
	    unsafe { wmidi::ControlValue::from_unchecked(63) }
	));

	assert!(region.is_active());
	assert_eq!(region.gain, 0.24607849215698431397);
    }

    #[test]
    fn note_trigger_release_key() {
	let mut rd = RegionData::default();
	rd.set_trigger(Trigger::ReleaseKey);
	let mut region = Region::new(rd, 1.0, 2);

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, unsafe { wmidi::Velocity::from_unchecked(63) }));
	assert!(!region.is_active());

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MAX));
	assert!(region.is_active());
	assert_eq!(region.gain, 0.24607849215698431397);
    }

    #[test]
    fn note_trigger_release_key_vel_range() {
	let mut rd = RegionData::default();
	rd.set_trigger(Trigger::ReleaseKey);
	rd.vel_range.set_hi(70).unwrap();
	rd.vel_range.set_lo(60).unwrap();
	let mut region = Region::new(rd, 1.0, 2);

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::try_from(90).unwrap()));
	assert!(!region.is_active());

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MIN));
	assert!(!region.is_active());


	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::try_from(63).unwrap()));
	assert!(!region.is_active());

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MIN));
	assert!(region.is_active());
	assert_eq!(region.gain, 0.24607849215698431397);
    }


    #[test]
    fn note_trigger_release_key_sustain_pedal() {
    	let mut rd = RegionData::default();
	rd.set_trigger(Trigger::ReleaseKey);
	let mut region = Region::new(rd, 1.0, 2);

	// sustain pedal on
	region.pass_midi_msg(&wmidi::MidiMessage::ControlChange(
	    wmidi::Channel::Ch1,
	    unsafe { wmidi::ControlNumber::from_unchecked(64) },
	    unsafe { wmidi::ControlValue::from_unchecked(64) }
	));

	// sustain pedal off
	region.pass_midi_msg(&wmidi::MidiMessage::ControlChange(
	    wmidi::Channel::Ch1,
	    unsafe { wmidi::ControlNumber::from_unchecked(64) },
	    unsafe { wmidi::ControlValue::from_unchecked(63) }
	));

	assert!(!region.is_active());

	// sustain pedal on
	region.pass_midi_msg(&wmidi::MidiMessage::ControlChange(
	    wmidi::Channel::Ch1,
	    unsafe { wmidi::ControlNumber::from_unchecked(64) },
	    unsafe { wmidi::ControlValue::from_unchecked(64) }
	));

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, unsafe { wmidi::Velocity::from_unchecked(63) }));
	assert!(!region.is_active());

	// sustain pedal off
	region.pass_midi_msg(&wmidi::MidiMessage::ControlChange(
	    wmidi::Channel::Ch1,
	    unsafe { wmidi::ControlNumber::from_unchecked(64) },
	    unsafe { wmidi::ControlValue::from_unchecked(63) }
	));

	assert!(!region.is_active());


	let mut rd = RegionData::default();
	rd.set_trigger(Trigger::ReleaseKey);
	let mut region = Region::new(rd, 1.0, 2);

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, unsafe { wmidi::Velocity::from_unchecked(63) }));
	assert!(!region.is_active());

    	// sustain pedal on
	region.pass_midi_msg(&wmidi::MidiMessage::ControlChange(
	    wmidi::Channel::Ch1,
	    unsafe { wmidi::ControlNumber::from_unchecked(64) },
	    unsafe { wmidi::ControlValue::from_unchecked(64) }
	));

	// sustain pedal off
	region.pass_midi_msg(&wmidi::MidiMessage::ControlChange(
	    wmidi::Channel::Ch1,
	    unsafe { wmidi::ControlNumber::from_unchecked(64) },
	    unsafe { wmidi::ControlValue::from_unchecked(63) }
	));

	assert!(!region.is_active());
    }

    #[test]
    fn note_trigger_first() {
	let mut rd = RegionData::default();
	rd.key_range.set_hi(60).unwrap();
	rd.key_range.set_lo(60).unwrap();
	rd.set_trigger(Trigger::First);
	let mut region = Region::new(rd, 1.0, 2);

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3,  wmidi::Velocity::MAX));
	assert!(region.is_active());

    	let mut rd = RegionData::default();
	rd.key_range.set_hi(60).unwrap();
	rd.key_range.set_lo(60).unwrap();
	rd.set_trigger(Trigger::First);
	let mut region = Region::new(rd, 1.0, 2);

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::A3,  wmidi::Velocity::MAX));
	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3,  wmidi::Velocity::MAX));
	assert!(!region.is_active());

        let mut rd = RegionData::default();
	rd.key_range.set_hi(60).unwrap();
	rd.key_range.set_lo(60).unwrap();
	rd.set_trigger(Trigger::First);
	let mut region = Region::new(rd, 1.0, 2);

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::A3,  wmidi::Velocity::MAX));
	region.pass_midi_msg(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::A3,  wmidi::Velocity::MAX));
	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3,  wmidi::Velocity::MAX));
	assert!(region.is_active());
    }

    #[test]
    fn note_trigger_legato() {
	let mut rd = RegionData::default();
	rd.key_range.set_hi(60).unwrap();
	rd.key_range.set_lo(60).unwrap();
	rd.set_trigger(Trigger::Legato);
	let mut region = Region::new(rd, 1.0, 2);

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3,  wmidi::Velocity::MAX));
	assert!(!region.is_active());

    	let mut rd = RegionData::default();
	rd.key_range.set_hi(60).unwrap();
	rd.key_range.set_lo(60).unwrap();
	rd.set_trigger(Trigger::Legato);
	let mut region = Region::new(rd, 1.0, 2);

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::A3,  wmidi::Velocity::MAX));
	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3,  wmidi::Velocity::MAX));
	assert!(region.is_active());

        let mut rd = RegionData::default();
	rd.key_range.set_hi(60).unwrap();
	rd.key_range.set_lo(60).unwrap();
	rd.set_trigger(Trigger::Legato);
	let mut region = Region::new(rd, 1.0, 2);

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::A3,  wmidi::Velocity::MAX));
	region.pass_midi_msg(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::A3,  wmidi::Velocity::MAX));
	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3,  wmidi::Velocity::MAX));
	assert!(!region.is_active());
    }

    #[test]
    fn note_off_sustain_pedal() {
	let rd = RegionData::default();
	let mut region = Region::new(rd, 1.0, 2);

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3,  wmidi::Velocity::MAX));
	assert!(region.is_active());

	// sustain pedal on
	region.pass_midi_msg(&wmidi::MidiMessage::ControlChange(
	    wmidi::Channel::Ch1,
	    unsafe { wmidi::ControlNumber::from_unchecked(64) },
	    unsafe { wmidi::ControlValue::from_unchecked(64) }
	));

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::C3,  wmidi::Velocity::MAX));
	assert!(region.is_active());

	// sustain pedal off
	region.pass_midi_msg(&wmidi::MidiMessage::ControlChange(
	    wmidi::Channel::Ch1,
	    unsafe { wmidi::ControlNumber::from_unchecked(64) },
	    unsafe { wmidi::ControlValue::from_unchecked(63) }
	));

	assert!(!region.is_active());
    }



    #[test]
    fn simple_note_on_off() {
	let sample = vec![0.1, -0.1,
			  0.2, -0.2,
			  0.3, -0.3,
			  0.4, -0.4,
			  0.5, -0.5];

	let mut engine = Engine::new(vec![RegionData::default()], 1.0, 16);

	engine.regions[0].set_sample_data(sample.clone());

	let mut out_left: [f32; 1] = [0.0];
	let mut out_right: [f32; 1] = [0.0];

	engine.process(&mut out_left, &mut out_right);

	assert_eq!(out_left[0], 0.0);
	assert_eq!(out_right[0], -0.0);

	let mut out_left: [f32; 1] = [0.0];
	let mut out_right: [f32; 1] = [0.0];

	engine.midi_event(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MAX));

	engine.process(&mut out_left, &mut out_right);
	assert_eq!(out_left[0], 0.1);
	assert_eq!(out_right[0], -0.1);

	let mut out_left: [f32; 1] = [0.0];
	let mut out_right: [f32; 1] = [0.0];

	engine.midi_event(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MAX));

	engine.process(&mut out_left, &mut out_right);

	assert_eq!(out_left[0], 0.0);
	assert_eq!(out_right[0], 0.0);
    }


    #[test]
    fn note_on_off_adsr() {
	let mut sample = vec![];
	sample.resize(48, 1.0);
	let regions = parse_sfz_text("<region> ampeg_attack=2 ampeg_hold=3 ampeg_decay=4 ampeg_sustain=60 ampeg_release=5".to_string()).unwrap();

	let mut engine = Engine::new(regions, 1.0, 16);

	engine.regions[0].set_sample_data(sample.clone());

	let mut out_left: [f32; 12] = [0.0; 12];
	let mut out_right: [f32; 12] = [0.0; 12];

	engine.midi_event(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MAX));
	engine.process(&mut out_left, &mut out_right);

	let out: Vec<f32> = out_left.iter().map(|v| (v*100.0).round()/100.0).collect();
	assert_eq!(out.as_slice(), [0.0, 0.5, 1.0, 1.0, 1.0, 0.65, 0.61, 0.6, 0.6, 0.6, 0.6, 0.6]);

	let mut out_left: [f32; 4] = [0.0; 4];
	let mut out_right: [f32; 4] = [0.0; 4];

	engine.process(&mut out_left, &mut out_right);

	let out: Vec<f32> = out_left.iter().map(|v| (v*10000.0).round()/10000.0).collect();
	assert_eq!(out.as_slice(), [0.6, 0.6, 0.6, 0.6]);

	engine.midi_event(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MAX));

	let mut out_left: [f32; 8] = [0.0; 8];
	let mut out_right: [f32; 8] = [0.0; 8];

	engine.process(&mut out_left, &mut out_right);

	let rel: Vec<f32> = out_left.iter().map(|v| (v*10000.0).round()/10000.0).collect();
	assert_eq!(rel.as_slice(), [0.1211, 0.0245, 0.0049, 0.0010, 0.0002, 0.0, 0.0, 0.0]);
    }


    #[test]
    fn note_on_velocity() {
	let sample = vec![1.0, 1.0];

	let mut region = Region::new(RegionData::default(), 1.0, 16);

	region.set_sample_data(sample.clone());

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::try_from(63).unwrap()));

	let mut out_left: [f32; 1] = [0.0];
	let mut out_right: [f32; 1] = [0.0];

	region.process(&mut out_left, &mut out_right);
	assert_eq!(out_left[0], 0.24607849215698431397);
	assert_eq!(out_right[0], 0.24607849215698431397);
    }

    #[test]
    fn note_on_gain_veltrack() {
	let sample = vec![1.0, 1.0];
	let mut rd = RegionData::default();
	rd.set_amp_veltrack(0.0).unwrap();

	let mut region = Region::new(rd, 1.0, 16);

	region.set_sample_data(sample.clone());

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MAX));

	let mut out_left: [f32; 1] = [0.0];
	let mut out_right: [f32; 1] = [0.0];

	region.process(&mut out_left, &mut out_right);
	assert_eq!(out_left[0], 1.0);
	assert_eq!(out_right[0], 1.0);


	region.pass_midi_msg(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MAX));
	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MIN));

	let mut out_left: [f32; 1] = [0.0];
	let mut out_right: [f32; 1] = [0.0];

	region.process(&mut out_left, &mut out_right);
	assert_eq!(out_left[0], 1.0);
	assert_eq!(out_right[0], 1.0);


	let mut rd = RegionData::default();
	rd.set_amp_veltrack(-100.0).unwrap();

	let mut region = Region::new(rd, 1.0, 16);

	region.set_sample_data(sample.clone());

	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MIN));

	let mut out_left: [f32; 1] = [0.0];
	let mut out_right: [f32; 1] = [0.0];

	region.process(&mut out_left, &mut out_right);
	assert_eq!(out_left[0], 1.0);
	assert_eq!(out_right[0], 1.0);


	region.pass_midi_msg(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MAX));
	region.pass_midi_msg(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MAX));

	let mut out_left: [f32; 1] = [0.0];
	let mut out_right: [f32; 1] = [0.0];

	region.process(&mut out_left, &mut out_right);
	assert_eq!(out_left[0], utils::dB_to_gain(-160.0));
	assert_eq!(out_right[0], utils::dB_to_gain(-160.0));

    }

    #[test]
    fn note_on_off_key_range() {
	let sample = vec![1.0, 1.0,
			  0.5, 0.5];

	let regions = parse_sfz_text("<region> lokey=60 hikey=60".to_string()).unwrap();

	let mut engine = Engine::new(regions, 1.0, 16);

	engine.regions[0].set_sample_data(sample.clone());

	engine.midi_event(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::A3, wmidi::Velocity::MAX));

	let mut out_left: [f32; 1] = [0.0];
	let mut out_right: [f32; 1] = [0.0];

	engine.process(&mut out_left, &mut out_right);
	assert!(f32_eq(out_left[0], 0.0));
	assert!(f32_eq(out_right[0], 0.0));

	engine.regions[0].set_sample_data(sample.clone());

	engine.midi_event(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MAX));

	let mut out_left: [f32; 1] = [0.0];
	let mut out_right: [f32; 1] = [0.0];

	engine.process(&mut out_left, &mut out_right);
	assert!(f32_eq(out_left[0], 1.0));
	assert!(f32_eq(out_right[0], 1.0));

	engine.midi_event(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::A3, wmidi::Velocity::MAX));

	let mut out_left: [f32; 1] = [0.0];
	let mut out_right: [f32; 1] = [0.0];

	engine.process(&mut out_left, &mut out_right);
	assert!(f32_eq(out_left[0], 0.5));
	assert!(f32_eq(out_right[0], 0.5));
    }

    #[test]
    fn note_on_off_multiple_regions_key() {
	let sample1 = vec![ 1e1,  1e2,  1e3,  1e4,  1e5,  1e6,  1e-1,  1e-2,  1e-3,  1e-4,  1e-5 , 1e-6];
	let sample2 = vec![                         2e5,  2e6,  2e-1,  2e-2,  2e-3,  2e-4,  2e-5 , 2e-6];
	let sample3 = vec![                         4e5,  4e6,  4e-1,  4e-2,  4e-3,  4e-4,  4e-5 , 4e-6];
	let sample4 = vec![            -8e3, -8e4, -8e5, -8e6, -8e-1, -8e-2, -8e-3, -8e-4, -8e-5 ,-8e-6];

	let region_text = "
<region> lokey=a3 hikey=a3 pitch_keycenter=57
<region> lokey=60 hikey=60 pitch_keycenter=60
<region> lokey=58 hikey=60 pitch_keycenter=60
<region> lokey=60 hikey=62 pitch_keycenter=61
".to_string();
	let regions = parse_sfz_text(region_text).unwrap();

	let mut engine = Engine::new(regions, 1.0, 1);

	engine.regions[0].set_sample_data(sample1.clone());
	engine.regions[1].set_sample_data(sample2.clone());
	engine.regions[2].set_sample_data(sample3.clone());
	engine.regions[3].set_sample_data(sample4.clone());

	for _ in 0..2 {
	    engine.midi_event(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::A1, wmidi::Velocity::MAX));

	    let mut out_left: [f32; 1] = [0.0];
	    let mut out_right: [f32; 1] = [0.0];

	    engine.process(&mut out_left, &mut out_right);
	    assert!(f32_eq(out_left[0], 0.0));
	    assert!(f32_eq(out_right[0], 0.0));

	    engine.midi_event(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::A2, wmidi::Velocity::MAX));

	    let mut out_left: [f32; 1] = [0.0];
	    let mut out_right: [f32; 1] = [0.0];

	    engine.process(&mut out_left, &mut out_right);
	    assert!(f32_eq(out_left[0], 1e1));
	    assert!(f32_eq(out_right[0], 1e2));

	    engine.midi_event(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::Db3, wmidi::Velocity::MAX));

	    let mut out_left: [f32; 1] = [0.0];
	    let mut out_right: [f32; 1] = [0.0];

	    engine.process(&mut out_left, &mut out_right);
	    assert!(f32_eq(out_left[0], -7e3));
	    assert!(f32_eq(out_right[0], -7e4));

	    engine.midi_event(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MAX));

	    let mut out_left: [f32; 1] = [0.0];
	    let mut out_right: [f32; 1] = [0.0];

	    engine.process(&mut out_left, &mut out_right);
	    assert!(f32_eq(out_left[0], -1e5));
	    assert!(f32_eq(out_right[0], -1e6));

	    engine.midi_event(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::A2, wmidi::Velocity::MAX));

	    let mut out_left: [f32; 1] = [0.0];
	    let mut out_right: [f32; 1] = [0.0];

	    engine.process(&mut out_left, &mut out_right);
	    assert!(f32_eq(out_left[0], -2e-1));
	    assert!(f32_eq(out_right[0], -2e-2));

	    engine.midi_event(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::Db3, wmidi::Velocity::MAX));

	    let mut out_left: [f32; 1] = [0.0];
	    let mut out_right: [f32; 1] = [0.0];

	    engine.process(&mut out_left, &mut out_right);
	    assert!(f32_eq(out_left[0], 6e-3));
	    assert!(f32_eq(out_right[0], 6e-4));

	    // no effect because sustaining
	    engine.midi_event(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::B2, wmidi::Velocity::MAX));

	    let mut out_left: [f32; 1] = [0.0];
	    let mut out_right: [f32; 1] = [0.0];

	    engine.process(&mut out_left, &mut out_right);
	    assert!(f32_eq(out_left[0], 6e-5));
	    assert!(f32_eq(out_right[0], 6e-6));

	    engine.midi_event(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MAX));

	    let mut out_left: [f32; 1] = [0.0];
	    let mut out_right: [f32; 1] = [0.0];

	    engine.process(&mut out_left, &mut out_right);
	    assert!(f32_eq(out_left[0], 0.0));
	    assert!(f32_eq(out_right[0], 0.0));
	}
    }

    #[test]
    fn note_on_off_multiple_regions_vel() {
	let sample1 = vec![ 1e1,  1e2,  1e3,  1e4,  1e5,  1e6,  1e-1,  1e-2,  1e-3,  1e-4,  1e-5 , 1e-6];
	let sample2 = vec![                         2e5,  2e6,  2e-1,  2e-2,  2e-3,  2e-4,  2e-5 , 2e-6];
	let sample3 = vec![                         4e5,  4e6,  4e-1,  4e-2,  4e-3,  4e-4,  4e-5 , 4e-6];
	let sample4 = vec![            -8e3, -8e4, -8e5, -8e6, -8e-1, -8e-2, -8e-3, -8e-4, -8e-5 ,-8e-6];

	let region_text = "
<region> lovel=30 hivel=30 amp_veltrack=0
<region> lovel=50 hivel=50 amp_veltrack=0
<region> lovel=40 hivel=50 amp_veltrack=0
<region> lovel=50 hivel=60 amp_veltrack=0
".to_string();
	let regions = parse_sfz_text(region_text).unwrap();

	let mut engine = Engine::new(regions, 1.0, 1);

	engine.regions[0].set_sample_data(sample1.clone());
	engine.regions[1].set_sample_data(sample2.clone());
	engine.regions[2].set_sample_data(sample3.clone());
	engine.regions[3].set_sample_data(sample4.clone());

	for _ in 0..2 {
	    engine.midi_event(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::try_from(20).unwrap()));

	    let mut out_left: [f32; 1] = [0.0];
	    let mut out_right: [f32; 1] = [0.0];

	    engine.process(&mut out_left, &mut out_right);
	    assert!(f32_eq(out_left[0], 0.0));
	    assert!(f32_eq(out_right[0], 0.0));

	    engine.midi_event(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::try_from(30).unwrap()));

	    let mut out_left: [f32; 1] = [0.0];
	    let mut out_right: [f32; 1] = [0.0];

	    engine.process(&mut out_left, &mut out_right);
	    assert!(f32_eq(out_left[0], 1e1));
	    assert!(f32_eq(out_right[0], 1e2));

	    engine.midi_event(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::try_from(55).unwrap()));

	    let mut out_left: [f32; 1] = [0.0];
	    let mut out_right: [f32; 1] = [0.0];

	    engine.process(&mut out_left, &mut out_right);
	    assert!(f32_eq(out_left[0], -7e3));
	    assert!(f32_eq(out_right[0], -7e4));

	    engine.midi_event(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::try_from(50).unwrap()));

	    let mut out_left: [f32; 1] = [0.0];
	    let mut out_right: [f32; 1] = [0.0];

	    engine.process(&mut out_left, &mut out_right);
	    assert!(f32_eq(out_left[0], -1e5));
	    assert!(f32_eq(out_right[0], -1e6));

	    engine.midi_event(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MIN));
	    engine.midi_event(&wmidi::MidiMessage::NoteOn(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::try_from(45).unwrap()));

	    let mut out_left: [f32; 1] = [0.0];
	    let mut out_right: [f32; 1] = [0.0];

	    engine.process(&mut out_left, &mut out_right);
	    assert!(f32_eq(out_left[0], 4e5));
	    assert!(f32_eq(out_right[0], 4e6));

	    engine.midi_event(&wmidi::MidiMessage::NoteOff(wmidi::Channel::Ch1, wmidi::Note::C3, wmidi::Velocity::MIN));

	    let mut out_left: [f32; 1] = [0.0];
	    let mut out_right: [f32; 1] = [0.0];

	    engine.process(&mut out_left, &mut out_right);
	    assert!(f32_eq(out_left[0], 0.0));
	    assert!(f32_eq(out_right[0], 0.0));
	}
    }


    #[test]
    fn pitch_keytrack_frequency() {
	let mut rd = RegionData::default();
	rd.pitch_keycenter = wmidi::Note::A3;
	//rd.set_pitch_keytrack(0.0);

	let mut region = Region::new(rd, 1.0, 2);

	region.note_on(wmidi::Note::A3, wmidi::Velocity::MAX);
	assert!(f32_eq(region.current_note_frequency as f32, 440.0));

	region.note_off();

	region.note_on(wmidi::Note::A4, wmidi::Velocity::MAX);
	assert!(f32_eq(region.current_note_frequency as f32, 880.0));

	let mut rd = RegionData::default();
	rd.pitch_keycenter = wmidi::Note::A3;
	rd.set_pitch_keytrack(0.0).unwrap();

	let mut region = Region::new(rd, 1.0, 2);

	region.note_on(wmidi::Note::A3, wmidi::Velocity::MAX);
	assert!(f32_eq(region.current_note_frequency as f32, 440.0));

	region.note_off();

	region.note_on(wmidi::Note::A4, wmidi::Velocity::MAX);
	assert!(f32_eq(region.current_note_frequency as f32, 440.0));

	let mut rd = RegionData::default();
	rd.pitch_keycenter = wmidi::Note::A3;
	rd.set_pitch_keytrack(-100.0).unwrap();

	let mut region = Region::new(rd, 1.0, 2);

	region.note_on(wmidi::Note::A3, wmidi::Velocity::MAX);
	assert!(f32_eq(region.current_note_frequency as f32, 440.0));

	region.note_off();

	region.note_on(wmidi::Note::A4, wmidi::Velocity::MAX);
	assert!(f32_eq(region.current_note_frequency as f32, 220.0));


	let mut rd = RegionData::default();
	rd.pitch_keycenter = wmidi::Note::A3;
	rd.set_pitch_keytrack(1200.0).unwrap();

	let mut region = Region::new(rd, 1.0, 2);

	region.note_on(wmidi::Note::A3, wmidi::Velocity::MAX);
	assert!(f32_eq(region.current_note_frequency as f32, 440.0));

	region.note_off();

	region.note_on(wmidi::Note::ASharp3, wmidi::Velocity::MAX);
	assert!(f32_eq(region.current_note_frequency as f32, 880.0));

    }

    #[test]
    fn tune_frequency() {
	let mut rd = RegionData::default();
	rd.tune = 1.0;

	let mut region = Region::new(rd, 1.0, 2);

	region.note_on(wmidi::Note::Ab3, wmidi::Velocity::MAX);
	assert!(f32_eq(region.current_note_frequency as f32, 440.0));

	let mut rd = RegionData::default();
	rd.tune = -1.0;

	let mut region = Region::new(rd, 1.0, 2);

	region.note_on(wmidi::Note::ASharp3, wmidi::Velocity::MAX);
	assert!(f32_eq(region.current_note_frequency as f32, 440.0));
    }

    /*


//    #[test]
    fn region_group() {
	let sample1 = vec![(0.1, -0.1), (0.1, -0.1), (0.1, -0.1), (0.1, -0.1), (0.1, -0.1)];
	let sample2 = vec![(0.2, -0.2), (0.2, -0.2), (0.2, -0.2), (0.2, -0.2), (0.2, -0.2)];
	let sample3 = vec![(0.3, -0.3), (0.3, -0.3), (0.3, -0.3), (0.3, -0.3), (0.3, -0.3)];

	let mut engine = Engine { regions: Vec::new() };

	let mut region = Region::default();
	region.sample_data = sample1.clone();
	region.set_group(1);

	engine.regions.push(region);

	let mut region = Region::default();
	region.sample_data = sample2.clone();
	region.set_group(1);

	engine.regions.push(region);

	let mut region = Region::default();
	region.sample_data = sample3.clone();

	engine.regions.push(region);

	let mut out_left: [f32; 2] = [0.0, 0.0];
	let mut out_right: [f32; 2] = [0.0, 0.0];

	engine.regions[0].state.position = Some(0);
	engine.regions[2].state.position = Some(0);
	engine.process(&mut out_left, &mut out_right);

	assert_eq!(out_left[0], 0.4);
	assert_eq!(out_right[0], -0.4);
	assert_eq!(out_left[1], 0.4);
	assert_eq!(out_right[1], -0.4);

	assert_eq!(engine.regions[0].state.position, Some(2));

	let mut out_left: [f32; 2] = [0.0, 0.0];
	let mut out_right: [f32; 2] = [0.0, 0.0];

	engine.regions[1].state.position = Some(0);
	engine.process(&mut out_left, &mut out_right);

	assert_eq!(out_left[0], 0.5);
	assert_eq!(out_right[0], -0.5);
	assert_eq!(out_left[1], 0.5);
	assert_eq!(out_right[1], -0.5);
	assert_eq!(engine.regions[0].state.position, None);

    }


*/
}