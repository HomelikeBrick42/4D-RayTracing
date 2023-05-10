#[derive(Clone, Copy)]
pub struct BiVector4 {
    pub xy: f32,
    pub xz: f32,
    pub xw: f32,
    pub yz: f32,
    pub yw: f32,
    pub zw: f32,
}

impl BiVector4 {
    pub const ZERO: BiVector4 = BiVector4 {
        xy: 0.0,
        xz: 0.0,
        xw: 0.0,
        yz: 0.0,
        yw: 0.0,
        zw: 0.0,
    };
    pub const XY: BiVector4 = BiVector4 {
        xy: 0.0,
        xz: 1.0,
        xw: 0.0,
        yz: 0.0,
        yw: 0.0,
        zw: 0.0,
    };
    pub const XZ: BiVector4 = BiVector4 {
        xy: 0.0,
        xz: 1.0,
        xw: 0.0,
        yz: 0.0,
        yw: 0.0,
        zw: 0.0,
    };
    pub const XW: BiVector4 = BiVector4 {
        xy: 0.0,
        xz: 0.0,
        xw: 1.0,
        yz: 0.0,
        yw: 0.0,
        zw: 0.0,
    };
    pub const YX: BiVector4 = BiVector4 {
        xy: -1.0,
        xz: 0.0,
        xw: 0.0,
        yz: 0.0,
        yw: 0.0,
        zw: 0.0,
    };
    pub const YZ: BiVector4 = BiVector4 {
        xy: 0.0,
        xz: 0.0,
        xw: 0.0,
        yz: 1.0,
        yw: 0.0,
        zw: 0.0,
    };
    pub const YW: BiVector4 = BiVector4 {
        xy: 0.0,
        xz: 0.0,
        xw: 0.0,
        yz: 0.0,
        yw: 1.0,
        zw: 0.0,
    };
    pub const ZX: BiVector4 = BiVector4 {
        xy: 0.0,
        xz: -1.0,
        xw: 0.0,
        yz: 0.0,
        yw: 0.0,
        zw: 0.0,
    };
    pub const ZY: BiVector4 = BiVector4 {
        xy: 0.0,
        xz: 0.0,
        xw: 0.0,
        yz: -1.0,
        yw: 0.0,
        zw: 0.0,
    };
    pub const ZW: BiVector4 = BiVector4 {
        xy: 0.0,
        xz: 0.0,
        xw: 0.0,
        yz: 0.0,
        yw: 0.0,
        zw: 1.0,
    };
    pub const WX: BiVector4 = BiVector4 {
        xy: 0.0,
        xz: 0.0,
        xw: -1.0,
        yz: 0.0,
        yw: 0.0,
        zw: 0.0,
    };
    pub const WY: BiVector4 = BiVector4 {
        xy: 0.0,
        xz: 0.0,
        xw: 0.0,
        yz: 0.0,
        yw: -1.0,
        zw: 0.0,
    };
    pub const WZ: BiVector4 = BiVector4 {
        xy: 0.0,
        xz: 0.0,
        xw: 0.0,
        yz: 0.0,
        yw: 0.0,
        zw: -1.0,
    };
}

impl BiVector4 {
    pub fn sqr_length(self) -> f32 {
        self.xy * self.xy
            + self.xz * self.xz
            + self.xw * self.xw
            + self.yz * self.yz
            + self.yw * self.yw
            + self.zw * self.zw
    }

    pub fn length(self) -> f32 {
        self.sqr_length().sqrt()
    }

    pub fn normalized(mut self) -> Self {
        let length = self.length();
        self.xy /= length;
        self.xz /= length;
        self.xw /= length;
        self.yz /= length;
        self.yw /= length;
        self.zw /= length;
        self
    }
}

impl std::ops::Neg for BiVector4 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            xy: -self.xy,
            xz: -self.xz,
            xw: -self.xw,
            yz: -self.yz,
            yw: -self.yw,
            zw: -self.zw,
        }
    }
}
