#[derive(Debug, Copy, Clone, PartialOrd, PartialEq)]
pub struct LogicalSize {
    pub w: f64,
    pub h: f64,
}

impl LogicalSize {
    pub fn width(&self) -> f64 {
        self.w
    }

    pub fn height(&self) -> f64 {
        self.h
    }

    pub fn from_physical(physical: PhysicalSize, dpi: Dpi) -> Self {
        Self {
            w: physical.w * dpi,
            h: physical.h * dpi,
        }
    }

    pub fn to_physical(&self, dpi: Dpi) -> PhysicalSize {
        PhysicalSize {
            w: self.w / dpi,
            h: self.h / dpi,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialOrd, PartialEq)]
pub struct PhysicalSize {
    pub w: f64,
    pub h: f64,
}

impl PhysicalSize {
    pub fn width(&self) -> f64 {
        self.w
    }

    pub fn height(&self) -> f64 {
        self.h
    }

    pub fn from_logical(logical: LogicalSize, dpi: Dpi) -> Self {
        Self {
            w: logical.w / dpi,
            h: logical.h / dpi,
        }
    }

    pub fn to_logical(&self, dpi: Dpi) -> LogicalSize {
        LogicalSize {
            w: self.w * dpi,
            h: self.h * dpi,
        }
    }
}

pub type Dpi = f64;
