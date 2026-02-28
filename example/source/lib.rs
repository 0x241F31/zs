use std::ffi::{self, c_void};
use std::sync::LazyLock;
use std::time::Duration;
use std::ptr::NonNull;

use velato::vello;
use vello::wgpu::rwh;
use vello::{kurbo, wgpu};

type AppResult<T = ()> = anyhow::Result<T>;
type JniResult<T = ()> = Result<T, jni::errors::Error>;

const NATIVE_METHODS: &[jni::NativeMethod] = &[
	jni::native_method! {
		java_type = "com.example.zs.Glue",
		static extern fn surface_created(surface: android.view.Surface, width: jint, height: jint),
	},
	jni::native_method! {
		java_type = "com.example.zs.Glue",
		static extern fn surface_changed(surface: android.view.Surface, width: jint, height: jint),
	},
	jni::native_method! {
		java_type = "com.example.zs.Glue",
		static extern fn surface_destroyed(surface: android.view.Surface),
	},
	jni::native_method! {
		java_type = "com.example.zs.Glue",
		static extern fn pause(),
	},
	jni::native_method! {
		java_type = "com.example.zs.Glue",
		static extern fn resume(),
	},
];

macro_rules! alog {
	($($arg:tt)*) => {{
		$crate::log(format!($($arg)*))
	}};
}

#[cfg(target_os = "android")]
fn configure_logging() {
	use tracing_subscriber::layer::SubscriberExt;
	use tracing_subscriber::util::SubscriberInitExt;

	let logger = android_logger::Config::default();
	let logger = logger.with_max_level(tracing::log::LevelFilter::Trace).with_tag("zs-libmain.so");
	android_logger::init_once(logger);

	_ = tracing_subscriber::registry().with(tracing_android_trace::AndroidTraceLayer::new()).try_init();
}

#[unsafe(export_name = "JNI_OnLoad")]
pub unsafe extern "system" fn constructor(vm: *mut jni::sys::JavaVM, _: *mut c_void) -> jni::sys::jint {
	static CTOR: std::sync::Once = std::sync::Once::new();
	CTOR.call_once(|| initialize(unsafe { jni::JavaVM::from_raw(vm) }).unwrap());
	jni::sys::JNI_VERSION_1_6
}

static ANIM: LazyLock<velato::Composition> =
	LazyLock::new(|| velato::Composition::from_slice(include_bytes!("../assets/crystal-ball.json")).unwrap());

fn initialize(vm: jni::JavaVM) -> JniResult {
	std::panic::set_hook(Box::new(|e| alog!("{}", e.to_string())));

	configure_logging();

	vm.with_top_local_frame(|env| -> JniResult {
		let class = env.find_class(jni::jni_str!("com/example/zs/Glue"))?;
		unsafe { env.register_native_methods(class, NATIVE_METHODS)? };
		Ok(())
	})?;

	log("methods were registered");

	#[cfg(debug_assertions)]
	log("DEBUG mode");

	#[cfg(not(debug_assertions))]
	log("RELEASE mode");

	Ok(())
}

fn surface_created<'l>(
	env: &mut jni::Env,
	_: jni::objects::JClass<'l>,
	surface: jni::objects::JObject,
	width: jni::sys::jint,
	height: jni::sys::jint,
) -> JniResult {
	unsafe {
		alog!("surface={} {width}x{height} created", surface.addr());

		let nw = ndk::native_window::from_surface(env.get_raw(), surface.cast());

		let ctx = init_wgpu(nw, width as u32, height as u32).unwrap();
		let data = Box::into_raw(Box::new(ctx)).cast::<ffi::c_void>();

		unsafe extern "C" fn on_frame(dt: i64, data: *mut ffi::c_void) {
			let mut ctx = unsafe { data.cast::<DrawContext>().as_mut_unchecked() };
			draw_scene(&mut ctx, dt).unwrap();
			unsafe { ndk::choreographer::post_frame_callback_64(ndk::choreographer::get_instance(), on_frame, data) };
		}
		ndk::choreographer::post_frame_callback_64(ndk::choreographer::get_instance(), on_frame, data);

		Ok(())
	}
}

struct DrawContext {
	device: wgpu::Device,
	queue: wgpu::Queue,
	config: wgpu::SurfaceConfiguration,
	surface: wgpu::Surface<'static>,

	vello: vello::Renderer,
	scene: vello::Scene,
	velato: velato::Renderer,

	blitter: wgpu::util::TextureBlitter,
	target_view: wgpu::TextureView,
}

fn init_wgpu(nw: *mut ndk::native_window::NativeWindow, width: u32, height: u32) -> AppResult<DrawContext> {
	let wgpu_desc = wgpu::InstanceDescriptor { backends: wgpu::Backends::VULKAN, ..Default::default() };
	let wgpu_instance = wgpu::Instance::new(&wgpu_desc);

	let display = rwh::AndroidDisplayHandle::new();
	let window = rwh::AndroidNdkWindowHandle::new(unsafe { NonNull::new_unchecked(nw.cast()) });

	let surface = unsafe {
		wgpu_instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
			raw_display_handle: display.into(),
			raw_window_handle: window.into(),
		})
	}?;

	let adapter = pollster::block_on(wgpu_instance.request_adapter(&wgpu::RequestAdapterOptions {
		power_preference: wgpu::PowerPreference::HighPerformance,
		compatible_surface: Some(&surface),
		force_fallback_adapter: false,
	}))?;

	let info = adapter.get_info();
	alog!(
		"adapter: name={} type={:?} backend={:?} driver={} driver_info={}",
		info.name,
		info.device_type,
		info.backend,
		info.driver,
		info.driver_info
	);

	let dev_desc = wgpu::DeviceDescriptor::default();
	let (device, queue) = pollster::block_on(adapter.request_device(&dev_desc))?;

	let capabilities = surface.get_capabilities(&adapter);
	alog!("surface capabilities: {capabilities:#?}");

	let format = capabilities
		.formats
		.into_iter()
		.find(|it| matches!(it, wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm))
		.ok_or(anyhow::anyhow!("unsupported format"))?;

	let config = wgpu::SurfaceConfiguration {
		usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
		format,
		width,
		height,
		present_mode: wgpu::PresentMode::AutoVsync,
		desired_maximum_frame_latency: 3,
		alpha_mode: wgpu::CompositeAlphaMode::Auto,
		view_formats: vec![],
	};

	let vello = vello::Renderer::new(
		&device,
		vello::RendererOptions {
			use_cpu: false,
			antialiasing_support: vello::AaSupport::area_only(),
			num_init_threads: None,
			pipeline_cache: None,
		},
	)?;
	let blitter = wgpu::util::TextureBlitter::new(&device, config.format);

	let target_texture = device.create_texture(&wgpu::TextureDescriptor {
		label: None,
		size: wgpu::Extent3d { width: config.width, height: config.height, depth_or_array_layers: 1 },
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: wgpu::TextureFormat::Rgba8Unorm,
		usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
		view_formats: &[],
	});
	let target_view = target_texture.create_view(&wgpu::TextureViewDescriptor::default());

	surface.configure(&device, &config);

	Ok(DrawContext {
		device,
		queue,
		config,
		surface,

		blitter,
		target_view,

		vello,
		scene: vello::Scene::new(),

		velato: velato::Renderer::new(),
	})
}

fn draw_scene(ctx: &mut DrawContext, dt: i64) -> AppResult {
	let time = Duration::from_nanos(dt as u64).as_secs_f64();
	let frame = ((time * ANIM.frame_rate) % (ANIM.frames.end - ANIM.frames.start)) + ANIM.frames.start;

	ctx.scene.reset();
	ctx.velato.append(&ANIM, frame, kurbo::Affine::IDENTITY, 1.0, &mut ctx.scene);

	let params = vello::RenderParams {
		width: ctx.config.width,
		height: ctx.config.height,
		base_color: vello::peniko::Color::BLACK,
		antialiasing_method: vello::AaConfig::Area,
	};
	ctx.vello.render_to_texture(&ctx.device, &ctx.queue, &ctx.scene, &ctx.target_view, &params)?;

	let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
	let surface_texture = ctx.surface.get_current_texture()?;
	ctx.blitter.copy(
		&ctx.device,
		&mut encoder,
		&ctx.target_view,
		&surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default()),
	);
	ctx.queue.submit([encoder.finish()]);
	surface_texture.present();

	_ = ctx.device.poll(wgpu::PollType::Poll);

	Ok(())
}

fn surface_changed<'l>(
	_: &mut jni::Env,
	_: jni::objects::JClass<'l>,
	surface: jni::objects::JObject,
	width: jni::sys::jint,
	height: jni::sys::jint,
) -> JniResult {
	alog!("surface={} {width}x{height} changed", surface.addr());
	Ok(())
}

fn surface_destroyed<'l>(_: &mut jni::Env, _: jni::objects::JClass<'l>, surface: jni::objects::JObject) -> JniResult {
	alog!("surface={} destroyed", surface.addr());
	Ok(())
}

fn pause<'l>(_: &mut jni::Env, _: jni::objects::JClass<'l>) -> JniResult {
	alog!("pause");
	Ok(())
}

fn resume<'l>(_: &mut jni::Env, _: jni::objects::JClass<'l>) -> JniResult {
	alog!("resume");
	Ok(())
}

pub fn log(msg: impl Into<String>) {
	#[link(name = "android")]
	unsafe extern "C" {
		#[link_name = "__android_log_write"]
		fn log_write(
			prio: core::ffi::c_int,
			tag: *const core::ffi::c_char,
			text: *const core::ffi::c_char,
		) -> core::ffi::c_int;
	}
	const ANDROID_LOG_INFO: core::ffi::c_int = 4;
	let tag = std::ffi::CString::new("libmain.so").unwrap();
	let msg = std::ffi::CString::new(msg.into()).unwrap();
	unsafe { log_write(ANDROID_LOG_INFO, tag.as_ptr(), msg.as_ptr()) };
}

mod ndk {
	pub mod choreographer {
		use std::ffi::c_void;

		#[repr(C)]
		pub struct Choreographer {
			_unused: [u8; 0],
		}

		#[link(name = "android")]
		unsafe extern "C" {
			#[link_name = "AChoreographer_getInstance"]
			pub fn get_instance() -> *mut Choreographer;

			#[link_name = "AChoreographer_postFrameCallback64"]
			pub fn post_frame_callback_64(
				cg: *mut Choreographer,
				callback: unsafe extern "C" fn(frame_time_ns: i64, data: *mut c_void),
				data: *mut c_void,
			);
		}
	}

	pub mod native_window {
		use std::ffi::c_void;

		#[repr(C)]
		pub struct NativeWindow {
			_unused: [u8; 0],
		}

		#[link(name = "android")]
		unsafe extern "C" {
			#[link_name = "ANativeWindow_fromSurface"]
			pub fn from_surface(env: *mut jni::sys::JNIEnv, surface: *mut c_void) -> *mut NativeWindow;
		}
	}
}
