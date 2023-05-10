use crate::BiVector4;
use cgmath::prelude::*;

#[derive(Clone, Copy)]
pub struct Rotor4 {
    pub s: f32,
    pub bv: BiVector4,
}

impl Rotor4 {
    pub const IDENTITY: Rotor4 = Rotor4 {
        s: 1.0,
        bv: BiVector4::ZERO,
    };
}

impl Rotor4 {
    pub fn from_rotation_between(from: cgmath::Vector4<f32>, to: cgmath::Vector4<f32>) -> Self {
        Rotor4 {
            s: 1.0 + to.dot(from),
            bv: wedge(to, from),
        }
        .normalized()
    }

    pub fn from_angle_plane(angle: f32, plane: BiVector4) -> Self {
        let half_angle = angle * 0.5;
        let (sin, cos) = half_angle.sin_cos();
        Self {
            s: cos,
            bv: BiVector4 {
                xy: plane.xy * -sin,
                xz: plane.xz * -sin,
                xw: plane.xw * -sin,
                yz: plane.yz * -sin,
                yw: plane.yw * -sin,
                zw: plane.zw * -sin,
            },
        }
        .normalized()
    }

    pub fn sqr_length(self) -> f32 {
        self.s * self.s + self.bv.sqr_length()
    }

    pub fn length(self) -> f32 {
        self.sqr_length().sqrt()
    }

    pub fn normalized(mut self) -> Self {
        let length = self.length();
        self.s /= length;
        self.bv.xy /= length;
        self.bv.xz /= length;
        self.bv.xw /= length;
        self.bv.yz /= length;
        self.bv.yw /= length;
        self.bv.zw /= length;
        self
    }

    #[rustfmt::skip]
    pub fn rotate_vec(self, v: cgmath::Vector4<f32>) -> cgmath::Vector4<f32> {
        let x = self.s * v.x + self.bv.xy * v.y + self.bv.xz * v.z + self.bv.xw * v.w;
        let y = self.s * v.y - self.bv.xy * v.x + self.bv.yz * v.z + self.bv.yw * v.w;
        let z = self.s * v.z - self.bv.xz * v.x - self.bv.yz * v.y + self.bv.zw * v.w;
        let w = self.s * v.w - self.bv.xw * v.x - self.bv.yw * v.y - self.bv.zw * v.z;

        let xyz = self.bv.xy * v.z - self.bv.xz * v.y + self.bv.yz * v.x;
        let yzw = self.bv.yz * v.w - self.bv.yw * v.z + self.bv.zw * v.y;
        let zwx = self.bv.xz * v.w - self.bv.xw * v.z + self.bv.zw * v.x;
        let wxy = self.bv.xy * v.w - self.bv.xw * v.y + self.bv.yw * v.x;

        let p = -self;
        cgmath::Vector4 {
            x: x * p.s - y * p.bv.xy - z * p.bv.xz - w * p.bv.xw - xyz * p.bv.yz - wxy * p.bv.yw - zwx * p.bv.zw,
            y: y * p.s + x * p.bv.xy - z * p.bv.yz - w * p.bv.yw + xyz * p.bv.xz + wxy * p.bv.xw - yzw * p.bv.zw,
            z: z * p.s + x * p.bv.xz + y * p.bv.yz - w * p.bv.zw - xyz * p.bv.xy + zwx * p.bv.xw + yzw * p.bv.yw,
            w: w * p.s + x * p.bv.xw + y * p.bv.yw + z * p.bv.zw - wxy * p.bv.xy - zwx * p.bv.xz - yzw * p.bv.yz,
        }
    }
}

impl std::ops::Neg for Rotor4 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            s: self.s,
            bv: -self.bv,
        }
    }
}

pub fn wedge(a: cgmath::Vector4<f32>, b: cgmath::Vector4<f32>) -> BiVector4 {
    BiVector4 {
        xy: (a.x * b.y) - (b.x * a.y),
        xz: (a.x * b.z) - (b.x * a.z),
        xw: (a.x * b.w) - (b.x * a.w),
        yz: (a.y * b.z) - (b.y * a.z),
        yw: (a.y * b.w) - (b.y * a.w),
        zw: (a.z * b.w) - (b.z * a.w),
    }
}
