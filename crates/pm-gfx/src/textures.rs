//! The pictures the frames are drawing, each uploaded as a texture of its own.
//!
//! A picture is uploaded the first frame that draws it and let go of the
//! first frame that does not, so a pane closed over a large image gives the
//! memory back without anyone having to say it was closed.

use std::collections::{HashMap, HashSet};

use crate::image::Image;

/// One uploaded picture, bound the way the image pipeline samples it.
struct Uploaded {
    /// The texture, kept alive for as long as the bind group names it.
    _texture: wgpu::Texture,
    /// The texture and its sampler, as the pipeline's second group.
    group: wgpu::BindGroup,
}

/// Every picture the last frames drew, by the picture's identity.
pub(crate) struct Textures {
    /// The layout every picture's bind group is made to.
    layout: wgpu::BindGroupLayout,
    /// How a picture is sampled: smoothly, so a scaled one does not shimmer.
    sampler: wgpu::Sampler,
    /// The pictures uploaded so far.
    uploaded: HashMap<u64, Uploaded>,
}

impl Textures {
    /// Nothing uploaded yet, on `device`.
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("picture"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("picture"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            layout,
            sampler,
            uploaded: HashMap::new(),
        }
    }

    /// The layout the image pipeline is built to.
    pub(crate) fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    /// Uploads every picture in `drawn` not uploaded yet, and lets go of
    /// every uploaded one it does not name.
    pub(crate) fn keep(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, drawn: &[&Image]) {
        let wanted = drawn.iter().map(|image| image.id()).collect::<HashSet<_>>();
        self.uploaded.retain(|id, _| wanted.contains(id));
        for image in drawn {
            if !self.uploaded.contains_key(&image.id()) {
                let uploaded = self.upload(device, queue, image);
                self.uploaded.insert(image.id(), uploaded);
            }
        }
    }

    /// The bind group of the picture called `id`, once it is uploaded.
    pub(crate) fn group(&self, id: u64) -> Option<&wgpu::BindGroup> {
        self.uploaded.get(&id).map(|uploaded| &uploaded.group)
    }

    /// Writes `image` into a texture of its own and binds it.
    fn upload(&self, device: &wgpu::Device, queue: &wgpu::Queue, image: &Image) -> Uploaded {
        let size = wgpu::Extent3d {
            width: image.width().max(1),
            height: image.height().max(1),
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("picture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            image.pixels(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * size.width),
                rows_per_image: Some(size.height),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("picture"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        Uploaded {
            _texture: texture,
            group,
        }
    }
}
