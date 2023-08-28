use bytemuck;

pub fn pixmap_pixels_wgpu_pixels(pix: &[u32]) -> &[u8] {
    panic!()
    // let out: &mut [u8] = bytemuck::cast_slice(pix)
    // for i in 0..out.len()/4 {
    //     let idx = 4 * i;
    //     let tmp1 = out[idx];
    //     let tmp2 = out[idx+1];
    //     out[idx] = out[idx + 3];
    //     out[idx+1] = out[idx + 2];
    //     out[idx + 2] = tmp2;
    //     out[idx + 3] = tmp1;
    // }
    // return out;
}

