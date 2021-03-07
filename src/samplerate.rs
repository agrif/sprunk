use libsamplerate_sys::*;

pub fn version() -> &'static str {
    let cstr = unsafe { std::ffi::CStr::from_ptr(src_get_version()) };
    cstr.to_str().unwrap() // assume libsamplerate uses valid utf-8
}

#[derive(Debug)]
pub struct SampleRate {
    state: *mut SRC_STATE,
    channels: u16,
}

impl Clone for SampleRate {
    fn clone(&self) -> Self {
        self.try_clone().unwrap() // this will *usually* work, barring OOM
    }
}

// implement this manually -- it's not automatic, because of the *mut SRC_STATE
// however, the way we use it, this is safe
unsafe impl Send for SampleRate {}

impl SampleRate {
    pub fn new(typ: ConverterType, channels: u16) -> Result<Self> {
        let mut errcode = 0;
        let state = unsafe {
            let staten = src_new(typ.to_c(), channels as libc::c_int, &mut errcode);
            Error::result(staten, errcode)
        }?;
        Ok(SampleRate { state, channels })
    }

    pub fn process(
        &mut self,
        ratio: f64,
        input: &[f32],
        output: &mut [f32],
    ) -> Result<(usize, usize)> {
        let channels = self.channels as libc::c_long;
        let mut cmd = SRC_DATA {
            data_in: input.as_ptr() as *const libc::c_float,
            data_out: output.as_mut_ptr() as *mut libc::c_float,
            input_frames: input.len() as libc::c_long / channels,
            output_frames: output.len() as libc::c_long / channels,
            input_frames_used: 0,
            output_frames_gen: 0,
            end_of_input: if input.len() == 0 { 1 } else { 0 },
            src_ratio: ratio as libc::c_double,
        };

        unsafe {
            let errcode = src_process(self.state, &mut cmd);
            Error::result((), errcode)?;
        }

        Ok((
            cmd.input_frames_used as usize * self.channels as usize,
            cmd.output_frames_gen as usize * self.channels as usize,
        ))
    }

    pub fn reset(&mut self) -> Result<()> {
        unsafe {
            let errcode = src_reset(self.state);
            Error::result((), errcode)
        }
    }

    pub fn try_clone(&self) -> Result<Self> {
        let state = unsafe {
            let mut errcode = 0;
            let staten = src_clone(self.state, &mut errcode);
            Error::result(staten, errcode)
        }?;
        Ok(SampleRate {
            state,
            channels: self.channels,
        })
    }

    pub fn channels(&self) -> usize {
        unsafe { src_get_channels(self.state) as usize }
    }

    pub fn set_ratio(&mut self, ratio: f64) -> Result<()> {
        unsafe {
            let errcode = src_set_ratio(self.state, ratio as libc::c_double);
            Error::result((), errcode)
        }
    }
}

impl Drop for SampleRate {
    fn drop(&mut self) {
        if !self.state.is_null() {
            unsafe {
                self.state = src_delete(self.state);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConverterType {
    SincBestQuality,
    SincMediumQuality,
    SincFastest,
    ZeroOrderHold,
    Linear,
}

impl ConverterType {
    pub fn name(&self) -> &'static str {
        let code = self.to_c() as i32;
        let cstr = unsafe { std::ffi::CStr::from_ptr(src_get_name(code)) };
        cstr.to_str().unwrap() // assume libsamplerate uses valid utf-8
    }

    pub fn description(&self) -> &'static str {
        let code = self.to_c() as i32;
        let cstr = unsafe { std::ffi::CStr::from_ptr(src_get_description(code)) };
        cstr.to_str().unwrap() // assume libsamplerate uses valid utf-8
    }

    fn to_c(&self) -> libc::c_int {
        use ConverterType::*;
        let code = match self {
            SincBestQuality => SRC_SINC_BEST_QUALITY,
            SincMediumQuality => SRC_SINC_MEDIUM_QUALITY,
            SincFastest => SRC_SINC_FASTEST,
            ZeroOrderHold => SRC_ZERO_ORDER_HOLD,
            Linear => SRC_LINEAR,
        };
        code as libc::c_int
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Error {
    BadCallback,
    BadChannelCount,
    BadConverter,
    BadData,
    BadDataPtr,
    BadInternalState,
    BadMode,
    BadPrivPtr,
    BadProcPtr,
    BadSincState,
    BadSrcRatio,
    BadState,
    DataOverlap,
    FilterLen,
    MallocFailed,
    NoPrivate,
    NoVariableRatio,
    NullCallback,
    ShiftBits,
    SincBadBufferLen,
    SincPrepareDataBadLen,
    SizeIncompatibility,
    Unknown(u32),
}

type Result<A> = std::result::Result<A, Error>;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        self.description()
    }
}

impl Error {
    pub fn description(&self) -> &'static str {
        let code = self.to_c();
        let cstr = unsafe { std::ffi::CStr::from_ptr(src_strerror(code)) };
        cstr.to_str().unwrap() // assume libsamplerate uses valid utf-8
    }

    fn result<A>(result: A, code: i32) -> Result<A> {
        if let Some(err) = Self::from_c(code) {
            Err(err)
        } else {
            Ok(result)
        }
    }

    fn to_c(&self) -> libc::c_int {
        use Error::*;
        // for some reason libsamplerate-sys doesn't expose this.
        let code = match self {
            MallocFailed => 1,
            BadState => 2,
            BadData => 3,
            BadDataPtr => 4,
            NoPrivate => 5,
            BadSrcRatio => 6,
            BadProcPtr => 7,
            ShiftBits => 8,
            FilterLen => 9,
            BadConverter => 10,
            BadChannelCount => 11,
            SincBadBufferLen => 12,
            SizeIncompatibility => 13,
            BadPrivPtr => 14,
            BadSincState => 15,
            DataOverlap => 16,
            BadCallback => 17,
            BadMode => 18,
            NullCallback => 19,
            NoVariableRatio => 20,
            SincPrepareDataBadLen => 21,
            BadInternalState => 22,
            Unknown(err) => *err,
        };
        code as libc::c_int
    }

    fn from_c(err: libc::c_int) -> Option<Self> {
        use Error::*;
        // for some reason libsamplerate-sys doesn't expose this.
        match err as u32 {
            0 => None,
            1 => Some(MallocFailed),
            2 => Some(BadState),
            3 => Some(BadData),
            4 => Some(BadDataPtr),
            5 => Some(NoPrivate),
            6 => Some(BadSrcRatio),
            7 => Some(BadProcPtr),
            8 => Some(ShiftBits),
            9 => Some(FilterLen),
            10 => Some(BadConverter),
            11 => Some(BadChannelCount),
            12 => Some(SincBadBufferLen),
            13 => Some(SizeIncompatibility),
            14 => Some(BadPrivPtr),
            15 => Some(BadSincState),
            16 => Some(DataOverlap),
            17 => Some(BadCallback),
            18 => Some(BadMode),
            19 => Some(NullCallback),
            20 => Some(NoVariableRatio),
            21 => Some(SincPrepareDataBadLen),
            22 => Some(BadInternalState),
            other => Some(Unknown(other)),
        }
    }
}
