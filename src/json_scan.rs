#[derive(Debug, Clone, Copy, PartialEq)]
enum ScanState {
    LookingForStart,
    InObject,
    InString,
    InEscape,
}

#[derive(Debug, PartialEq)]
pub enum ScanResult {
    NeedsMore,
    Error,
    Found(usize), // Split point in the input slice
}

pub struct JsonScanner {
    state: ScanState,
    brace_depth: usize,
}

impl JsonScanner {
    pub fn new() -> Self {
        Self {
            state: ScanState::LookingForStart,
            brace_depth: 0,
        }
    }

    pub fn scan(&mut self, input: &[u8]) -> ScanResult {
        for (i, &byte) in input.iter().enumerate() {
            match self.state {
                ScanState::LookingForStart => {
                    if byte.is_ascii_whitespace() {
                        continue;
                    } else if byte == b'{' {
                        self.state = ScanState::InObject;
                        self.brace_depth = 1;
                    } else {
                        return ScanResult::Error;
                    }
                }

                ScanState::InObject => match byte {
                    b'{' => self.brace_depth += 1,
                    b'}' => {
                        self.brace_depth -= 1;
                        if self.brace_depth == 0 {
                            self.reset();
                            return ScanResult::Found(i + 1);
                        }
                    }
                    b'"' => self.state = ScanState::InString,
                    _ => {}
                },

                ScanState::InString => match byte {
                    b'"' => self.state = ScanState::InObject,
                    b'\\' => self.state = ScanState::InEscape,
                    _ => {}
                },

                ScanState::InEscape => {
                    self.state = ScanState::InString;
                }
            }
        }

        ScanResult::NeedsMore
    }

    fn reset(&mut self) {
        self.state = ScanState::LookingForStart;
        self.brace_depth = 0;
    }
}
