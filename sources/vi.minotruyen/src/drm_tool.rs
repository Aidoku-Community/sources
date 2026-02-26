use aidoku::{
	ImageResponse, PageContext,
	alloc::{
		borrow::ToOwned,
		boxed::Box,
		string::{String, ToString},
		sync::Arc,
	},
	canvas::Rect,
	imports::canvas::{Canvas, ImageRef},
	prelude::*,
};
use anyhow::{Context, Result};
use once_cell::race::OnceBox;
use spin::Mutex;
use wasmi::*;

use crate::env::PHRASE_KEY;

#[derive(Clone, Debug)]
struct Element {
	pub inner_html: String,
}
static PHRASE_ELEMENT: OnceBox<Element> = OnceBox::new();

#[derive(Debug)]
struct ImageElement {
	pub width: usize,
	pub image: ImageRef,
}

#[derive(Debug)]
struct CanvasRenderingContext2D {
	// Option hack take box
	pub canvas: Arc<Mutex<Option<Canvas>>>,
}

fn init() {
	PHRASE_ELEMENT.get_or_init(|| {
		Box::new(Element {
			inner_html: PHRASE_KEY.to_owned(),
		})
	});
}

#[repr(u8)]
#[derive(Clone, Debug)]
enum GlobalVal {
	Selff = 1,
	Global,
	Window,
	Document,
	GlobalThis,
}

#[derive(Default)]
struct WasmStore {
	pub table: Option<Table>,
	pub table_alloc_fn: Option<TypedFunc<(), i32>>,
}

fn ext(caller: &mut Caller<'_, WasmStore>, obj: impl Into<String>) -> ExternRef {
	ExternRef::new(caller, obj.into())
}

// fn read_memory(caller: &Caller<'_, WasmStore>, ptr: usize, len: usize) -> anyhow::Result<Vec<u8>> {
// 	// メモリを取得
// 	let memory = caller
// 		.get_export("memory")
// 		.and_then(|e| e.into_memory())
// 		.ok_or_else(|| anyhow::anyhow!("memory export not found"))?;

// 	let buffer = &memory.data(caller)[ptr..(ptr + len)];
// 	// メモリアクセス
// 	// memory.read(caller, ptr, &mut buffer)?;
// 	// .ok_or_else(|| anyhow::anyhow!("invalid memory range"))?;

// 	Ok(buffer.to_vec())
// }

fn set_table<T>(mut caller: Caller<'_, WasmStore>, value: T) -> Result<i32>
where
	T: 'static + core::any::Any + Send + Sync,
{
	let index = {
		let store = caller.data();
		store.table_alloc_fn.unwrap().call(&mut caller, ())? as u64
	};

	let value = ExternRef::new(&mut caller, value).into();

	let table = {
		let store = caller.data();
		store.table.unwrap()
	};

	if table
		.get(&mut caller, index)
		.map(|v| v.externref().map(|v| v.is_null()).unwrap_or(true))
		.unwrap_or(true)
	{
		table.set(&mut caller, index, value)?;
	} else {
		println!("PANIC: table index {} exists!!!", index);
	}

	Ok(index as i32)
}

fn pass_string_to_wasm(
	store: &mut Store<WasmStore>,
	instance: &Instance,
	s: &str,
) -> anyhow::Result<(i32, i32)> {
	let mut store = store;
	// メモリを取得
	let memory = instance
		.get_export(&store, "memory")
		.and_then(|e| e.into_memory())
		.expect("memory export missing");

	// malloc 関数を取得
	let malloc = instance
		.get_export(&store, "__wbindgen_malloc")
		.and_then(|e| e.into_func())
		.expect("__wbindgen_malloc not found");

	// realloc 関数を取得
	// let realloc = instance
	//     .get_export(store, "__wbindgen_realloc")
	//     .and_then(|e| e.into_func())
	//     .expect("__wbindgen_realloc not found");

	let utf8 = s.as_bytes().to_vec();
	let len = utf8.len() as i32;

	// まず malloc によるアロケート
	// __wbindgen_malloc(size, align)
	let mut results = [Val::I32(0)];
	{
		malloc.call(&mut store, &[Val::I32(len), Val::I32(1)], &mut results)?;
	}

	let ptr = results[0].i32().context("invalid ptr")?;

	{
		// メモリへ書き込み
		let mem = memory.data_mut(&mut store);

		let end = (ptr as usize) + (len as usize);
		mem[ptr as usize..end].copy_from_slice(&utf8);
	}

	// 文字列にマルチバイトが含まれるなどで realloc が必要ならここで処理する
	// wasm-bindgen JS と違って Rust 側で UTF-8 の長さは分かっている。
	// なので realloc は普通は不要。
	// でも wasm-bindgen の動作と同じにしたければここで可能。

	Ok((ptr, len))
}

fn pass_string_to_wasm_caller(
	mut caller: &mut Caller<'_, WasmStore>,
	s: &str,
) -> anyhow::Result<(i32, i32)> {
	let memory = caller
		.get_export("memory")
		.and_then(|e| e.into_memory())
		.ok_or_else(|| anyhow::anyhow!("memory export missing"))?;

	let malloc = caller
		.get_export("__wbindgen_malloc")
		.and_then(|e| e.into_func())
		.ok_or_else(|| anyhow::anyhow!("__wbindgen_malloc not found"))?;

	let utf8 = s.as_bytes();
	let len = utf8.len() as i32;

	let mut results = [Val::I32(0)];
	{
		malloc.call(&mut caller, &[Val::I32(len), Val::I32(1)], &mut results)?
	}

	let ptr = results[0].i32().context("invalid ptr")?;
	{
		// メモリへ書き込み
		let mem = memory.data_mut(&mut caller);

		let end = (ptr as usize) + (len as usize);
		mem[ptr as usize..end].copy_from_slice(utf8);
	}

	Ok((ptr, len))
}

fn register_linker(linker: &mut Linker<WasmStore>) -> anyhow::Result<()> {
	// (param externref) -> i32
	linker.func_new(
		"./drm_tool_bg.js",
		"__wbg_instanceof_Window_def73ea0955fc569",
		wasmi::FuncType::new([ValType::ExternRef], [ValType::I32]),
		|mut caller, params, results| {
			let v = params[0].externref();
			let is_window = v
				.and_then(|x| {
					x.val()
						.unwrap()
						.data(&mut caller)
						.downcast_ref::<GlobalVal>()
				})
				.map(|v| matches!(v, GlobalVal::Selff | GlobalVal::Global | GlobalVal::Window))
				.unwrap_or_default();

			println!("__wbg_instanceof_Window_def73ea0955fc569 {:?}", is_window);

			results[0] = Val::I32(if is_window { 1 } else { 0 });
			Ok(())
		},
	)?;

	// ==== innerHTML setter ====
	linker.func_new(
		"./drm_tool_bg.js",
		"__wbg_innerHTML_e1553352fe93921a",
		FuncType::new([ValType::I32, ValType::ExternRef], []),
		|mut caller, params, _results| {
			println!("__wbg_innerHTML_e1553352fe93921a {:?}", params);

			let inner_html = {
				&params[1]
					.externref()
					.unwrap()
					.val()
					.unwrap()
					.data(&caller)
					.downcast_ref::<Element>()
					.unwrap()
					.inner_html
					.clone()
			};
			println!("HTML is {:?}", inner_html);

			let (new_ptr, len) = pass_string_to_wasm_caller(&mut caller, inner_html).unwrap();
			let ptr = params[0].i32().unwrap() as usize;

			let memory = caller
				.get_export("memory")
				.and_then(|e| e.into_memory())
				.ok_or_else(|| anyhow::anyhow!("memory export missing"))
				.unwrap()
				.data_mut(&mut caller);

			println!("memory size = {:?}", memory.len());

			// write man
			memory[ptr..(ptr + 4)].copy_from_slice(&new_ptr.to_le_bytes());
			memory[(ptr + 4)..(ptr + 8)].copy_from_slice(&len.to_le_bytes());

			// memory
			// if let Some(x) = el
			//     && let Some(s) = x.val().unwrap().data(&mut caller).downcast_ref::<String>()
			// {
			//     println!("Set innerHTML of {s}");
			// }
			// no return value
			Ok(())
		},
	)?;

	linker.func_new(
		"./drm_tool_bg.js",
		"__wbg_drawImage_07c37f8560e58bbd",
		FuncType::new(
			[
				ValType::ExternRef, // CanvasRenderingContext2D
				ValType::ExternRef, // ImageElement
				ValType::F64,
				ValType::F64,
				ValType::F64,
				ValType::F64,
				ValType::F64,
				ValType::F64,
				ValType::F64,
				ValType::F64,
			],
			[],
		),
		|caller, params, _results| {
			println!("{:?} __wbg_drawImage_07c37f8560e58bbd", params);

			let ctx = params[0]
				.externref()
				.unwrap()
				.val()
				.unwrap()
				.data(&caller)
				.downcast_ref::<CanvasRenderingContext2D>()
				.unwrap();
			let img = params[1]
				.externref()
				.unwrap()
				.val()
				.unwrap()
				.data(&caller)
				.downcast_ref::<ImageElement>()
				.unwrap();
			let sx = params[2].f64().unwrap().to_float() as f32;
			let sy = params[3].f64().unwrap().to_float() as f32;
			let s_width = params[4].f64().unwrap().to_float() as f32;
			let s_height = params[5].f64().unwrap().to_float() as f32;
			let dx = params[6].f64().unwrap().to_float() as f32;
			let dy = params[7].f64().unwrap().to_float() as f32;
			let d_width = params[8].f64().unwrap().to_float() as f32;
			let d_height = params[9].f64().unwrap().to_float() as f32;

			let src_rect = Rect::new(sx, sy, s_width, s_height);
			let des_rect = Rect::new(dx, dy, d_width, d_height);

			let mut ctx = ctx.canvas.lock();

			let canvas = ctx.as_mut().unwrap();
			canvas.copy_image(&img.image, src_rect, des_rect);

			println!("drawImage called: ctx={ctx:?}, img={img:?}");

			Ok(())
		},
	)?;

	// ==== width getter ====
	linker.func_new(
		"./drm_tool_bg.js",
		"__wbg_width_4f334fc47ef03de1",
		wasmi::FuncType::new([ValType::ExternRef], [ValType::I32]),
		|mut caller, params, results| {
			let el = params[0].externref();

			// example logic
			let width = if let Some(x) = el {
				if let Some(s) = x
					.val()
					.unwrap()
					.data(&mut caller)
					.downcast_ref::<ImageElement>()
				{
					s.width as i32
					// if s.starts_with("el(") { 640 } else { 0 }
				} else {
					println!("Can't image element");
					0
				}
			} else {
				0
			};

			results[0] = Val::I32(width);
			Ok(())
		},
	)?;

	// ==== new Function() ====
	linker.func_new(
		"./drm_tool_bg.js",
		"__wbg_newnoargs_105ed471475aaf50",
		wasmi::FuncType::new([ValType::I32, ValType::I32], [ValType::ExternRef]),
		|mut caller, _, results| {
			println!("__wbg_newnoargs_105ed471475aaf50");

			// let pointer: usize = params[0].i32().unwrap() as usize >> 0;
			// let length: usize = params[1].i32().unwrap() as usize;

			// let js = String::from_utf8(read_memory(&caller, pointer, length).unwrap()).unwrap();

			// placeholder
			results[0] = Val::FuncRef(
				Func::wrap(&mut caller, || {
					println!("callback call");

					Ok(())
				})
				.into(),
			);

			Ok(())
		},
	)?;

	// ==== is undefined ====
	linker.func_new(
		"./drm_tool_bg.js",
		"__wbindgen_is_undefined",
		wasmi::FuncType::new([ValType::ExternRef], [ValType::I32]),
		|_, params, results| {
			// let is_undefined = params[0]
			//     .externref()
			//     .map(|v| v.val().is_none() || v.is_null())
			//     .unwrap_or(true);

			let is_undefined = params[0].externref().is_none();

			println!("__wbindgen_is_undefined {:?}", is_undefined);

			results[0] = Val::I32(if is_undefined { 1 } else { 0 });
			Ok(())
		},
	)?;

	// ==== call() ====
	linker.func_new(
		"./drm_tool_bg.js",
		"__wbg_call_672a4d21634d4a24",
		wasmi::FuncType::new(
			[ValType::ExternRef, ValType::ExternRef],
			[ValType::ExternRef],
		),
		|mut caller, params, results| {
			println!("{:?} __wbg_call_672a4d21634d4a24", params);
			let func = params[0].externref().unwrap();
			let param = params[0].externref().unwrap();

			// let func = { func.val().unwrap().data(&mut caller) };
			// let param = { param.val().unwrap().data(&mut caller) };
			println!("{:?} => {:?}", func, param);

			// fake callable result
			results[0] = Val::ExternRef(ext(&mut caller, "call_result").into());
			Ok(())
		},
	)?;

	// ==== throw ====
	linker.func_new(
		"./drm_tool_bg.js",
		"__wbindgen_throw",
		wasmi::FuncType::new([ValType::I32, ValType::I32], []),
		|_caller, params, _| {
			println!("{:?} __wbindgen_throw", params);

			let ptr = params[0].i32().unwrap();
			let len = params[1].i32().unwrap();
			println!("WASM threw: ptr={ptr} len={len}");
			Ok(())
		},
	)?;

	// ==== debug string ====
	linker.func_new(
		"./drm_tool_bg.js",
		"__wbindgen_debug_string",
		wasmi::FuncType::new([ValType::I32, ValType::ExternRef], []),
		|mut caller, params, _| {
			println!("{:?} __wbindgen_debug_string", params);

			let _ptr = params[0].i32();
			let er = params[1].externref();
			if let Some(x) = er {
				println!(
					"debug: {:?}",
					x.val().unwrap().data(&mut caller).downcast_ref::<String>()
				);
			}
			Ok(())
		},
	)?;

	// ==== externref table init ====
	linker.func_new(
		"./drm_tool_bg.js",
		"__wbindgen_init_externref_table",
		wasmi::FuncType::new([], []),
		|mut caller, _, _| {
			println!("externref table initialized");

			let table = {
				let store = caller.data();
				store.table.unwrap()
			};

			for index in 0..(table.size(&mut caller) - 1) {
				table.set(&mut caller, index, Val::default(ValType::ExternRef))?;
			}

			let n = table.grow(&mut caller, 4, Val::default(ValType::ExternRef))?;

			table.set(&mut caller, 0, Val::default(ValType::ExternRef))?;
			table.set(&mut caller, n, Val::default(ValType::ExternRef))?;
			table.set(&mut caller, n + 1, Val::default(ValType::ExternRef))?;
			let v = ExternRef::new(&mut caller, true);
			table.set(&mut caller, n + 2, Val::ExternRef(v.into()))?;
			let v = ExternRef::new(&mut caller, false);
			table.set(&mut caller, n + 3, Val::ExternRef(v.into()))?;

			Ok(())
		},
	)?;

	Ok(())
}

pub struct DrmToolWasm {
	instance: Instance,
	store: Store<WasmStore>,
}
impl DrmToolWasm {
	pub fn new() -> anyhow::Result<Self> {
		init();

		// Engine
		let engine = Engine::default();

		// Load wasm
		let module = Module::new(&engine, include_bytes!("8ccda4605051db32.wasm"))?;

		let mut store = Store::new(&engine, WasmStore::default());
		let mut linker = Linker::new(&engine);

		register_linker(&mut linker)?;

		// ==== SELF / GLOBAL / WINDOW ====
		for (name, val) in [
			(
				"__wbg_static_accessor_SELF_37c5d418e4bf5819",
				GlobalVal::Selff,
			),
			(
				"__wbg_static_accessor_GLOBAL_88a902d13a557d07",
				GlobalVal::Global,
			),
			(
				"__wbg_static_accessor_WINDOW_5de37043a91a9c40",
				GlobalVal::Window,
			),
			(
				"__wbg_static_accessor_GLOBAL_THIS_56578be7e9f832b0",
				GlobalVal::GlobalThis,
			),
		] {
			linker.func_new(
				"./drm_tool_bg.js",
				name,
				wasmi::FuncType::new([], [ValType::I32]),
				move |caller, _, results| {
					println!("{}", name);

					let index = set_table(caller, val.clone()).unwrap();
					results[0] = Val::I32(index);

					Ok(())
				},
			)?;
		}

		linker.func_new(
			"./drm_tool_bg.js",
			"__wbg_document_d249400bd7bd996d",
			wasmi::FuncType::new([ValType::ExternRef], [ValType::I32]),
			|caller, _, results| {
				println!("__wbg_document_d249400bd7bd996d");

				let index = set_table(caller, GlobalVal::Document).unwrap();
				println!("document is {:?}", index);

				results[0] = Val::I32(index);

				Ok(())
			},
		)?;

		// ==== querySelector(document, ptr, len) ====
		linker.func_new(
			"./drm_tool_bg.js",
			"__wbg_querySelector_c69f8b573958906b",
			wasmi::FuncType::new(
				[ValType::ExternRef, ValType::I32, ValType::I32],
				[ValType::I32],
			),
			|caller, params, results| {
				// let document = params[0].externref().unwrap().val().unwrap().data(&mut caller).downcast_ref::<GlobalVal>();
				let ptr = params[1].i32().unwrap() as usize;
				let len = params[2].i32().unwrap() as usize;

				let mem = caller.get_export("memory").unwrap().into_memory().unwrap();

				// Japanese comment: WasmメモリからCSSセレクタ文字列を読み込む
				let selector = {
					let mut buf = crate::vec![0u8; len];
					mem.read(&caller, ptr, &mut buf)?;
					String::from_utf8_lossy(&buf).to_string()
				};

				if selector != "#phrase" {
					println!("Error not support element != #phrase");
				}

				let index =
					set_table(caller, PHRASE_ELEMENT.get().unwrap().clone() as Element).unwrap();
				results[0] = Val::I32(index);

				println!("__wbg_querySelector_c69f8b573958906b (index = {:?})", index);

				Ok(())
			},
		)?;

		// Instantiate and start
		let instance = linker.instantiate_and_start(&mut store, &module)?;

		let table = instance
			.get_export(&store, "__wbindgen_export_2")
			.and_then(Extern::into_table)
			.ok_or_else(|| anyhow::anyhow!("export __wbindgen_export_2 not found"))?;
		let table_alloc_fn =
			instance.get_typed_func::<(), i32>(&store, "__externref_table_alloc")?;
		{
			let store = store.data_mut();

			store.table_alloc_fn = Some(table_alloc_fn);
			store.table = Some(table);
		}
		Ok(Self { instance, store })
	}

	// fn __externref_table_alloc(&mut self) -> Result<i32> {
	//     let table_alloc_fn = self
	//         .instance
	//         .get_typed_func::<(), i32>(&self.store, "__externref_table_alloc")?;

	//     let value = table_alloc_fn.call(&mut self.store, ())?;

	//     Ok(value)
	// }

	pub fn start(&mut self) -> Result<()> {
		let start_fn = self
			.instance
			.get_typed_func::<(), ()>(&self.store, "__wbindgen_start")?;
		start_fn.call(&mut self.store, ())?;

		Ok(())
	}

	pub fn render_image(
		&mut self,
		response: ImageResponse,
		context: Option<&PageContext>,
	) -> Result<ImageRef> {
		let Some(context) = context else {
			return Err(anyhow::anyhow!("Biribiri!!!!!"));
		};

		let width = context
			.get("width")
			.and_then(|w| w.parse::<usize>().ok())
			.unwrap_or_default();
		let height = context
			.get("height")
			.and_then(|h| h.parse::<usize>().ok())
			.unwrap_or_default();
		let drm_data = context
			.get("drm_data")
			.map(|drm| drm.replace("\n", ""))
			.unwrap_or_default();

		if response.request.url.is_none() || drm_data.is_empty() {
			return Ok(response.image);
		};

		let canvas = Arc::new(Mutex::new(Some(Canvas::new(width as f32, height as f32))));

		let func = self.instance.get_func(&self.store, "render_image").unwrap();
		let img = ExternRef::new(
			&mut self.store,
			ImageElement {
				width,
				image: response.image,
			},
		);

		let ctx = ExternRef::new(
			&mut self.store,
			CanvasRenderingContext2D {
				canvas: canvas.clone(),
			},
		);
		let (ptr, len) = pass_string_to_wasm(&mut self.store, &self.instance, &drm_data)?;

		// Gọi hàm
		func.call_resumable(
			&mut self.store,
			&[
				Val::ExternRef(img.into()),
				Val::ExternRef(ctx.into()),
				Val::I32(ptr),
				Val::I32(len),
			],
			&mut [],
		)?;

		Ok(canvas.lock().take().unwrap().get_image())
	}
}

// fn main() -> anyhow::Result<()> {
// 	let mut drm = DrmToolWasm::new()?;

// 	drm.start()?;
// 	// drm.render_image()?;

// 	println!("ok");

// 	Ok(())
// }
