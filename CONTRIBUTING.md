# Contributing

Thank you for your interest in contributing to the Aidoku community sources!

If you are new to Aidoku source development, please consult [the official source development guide](https://aidoku.github.io/aidoku-rs/book/). This document outlines expectations for code submitted to this repo, and provides some general tips to supplement the official documentation.

## Contribution expectations

One of the things you may notice about this repo is that we utilize [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) to organize commits, and that all contributions are done through PRs that are squashed on merge. Setting a PR title to a good commit message helps us out. Here are some examples:
- `feat: add en.mangago`
- `fix(vi.cmanga): update url`
- `feat(en.comix): add filtering and home listings`
- `chore: remove en.mangapark`

This also means that any unrelated changes, or changes that are not dependent on each other, should often be split into separate PRs. For example, a single PR usually should not add two separate, unrelated sources at the same time, since they can be reviewed and merged individually instead.

All PRs will have actions run on them to ensure they meet basic standards before review, and some formatting will be checked manually in reviews. To ensure your PRs pass this stage, please work through the following checklist:
- [ ] The source can be compiled without any warnings or errors.
- [ ] `cargo fmt` has been run before submission.
- [ ] `cargo clippy` outputs no lint warnings.
- [ ] All files have an additional newline at the end.
- [ ] JSON files use tabs for indentation.

### Minimum source functionality

All sources should generally satisfy the following requirements:
- `status`, `content_rating`, and `viewer` attributes are set on each series when fetching details, when the source contains the relevant data.
	- `status` corresponds to the current publishing status of a series.
	- `content_rating` can be determined using tags even if the source doesn't have its own content rating classifications.
	- `viewer` can be determined if the source classifies a series as a manhwa, manhua, or webtoon, or if the source only contains a particular content type.
- `send_partial_result` is used in `get_manga_list` if fetching chapters requires an additional network request.
- `DeepLinkHandler` is implemented for handling series and chapter urls, if possible.
- Built-in language filtering is used for multi-language sources instead of setting the language as "multi", if supported.
- The filters available on the source website have corresponding filter options in the source.

Implementing home pages and listings is not necessary for all sources, but is encouraged if you feel inclined to do so.

### AI usage

Using AI for assistance or reference when creating sources is allowed, but is generally not acceptable if no human thought is put into the resulting code. AI tools tend to either generate needlessly complex code, or incorrect code that's hard to spot at first glance. This greatly increases the effort required for reviewing.

Learning to use Rust and develop Aidoku sources is fun and feels more accomplishing to do without AI assistance. We are happy to provide reviews suggesting how to improve your code so you can accelerate your learning, and the [Aidoku Discord server](https://discord.gg/aidoku/) is additionally available to help in the `#source-dev` channel.

## Templates

Some websites tend to have very similar structures, especially those that use [WordPress](https://wordpress.org/) themes. In those cases, we write [templates](https://github.com/Aidoku-Community/sources/tree/main/templates) that can be used for multiple sources. 

There are two useful resources for checking if a source should use a template. Firstly, check if there is an existing Mihon source for the website available from [Keiyoushi Extensions](https://github.com/keiyoushi/extensions-source). If the Mihon source has a "themePkg", then it's likely we should have a matching source template. Otherwise, you can try using [wpthemedetector.com](https://www.wpthemedetector.com/) to see if a website is using a WordPress theme.

When creating a new template, you can reference any of the existing templates. Templates should provide a `Params` struct containing configuration parameters for individual sources, and an `Impl` trait containing default function implementations that use the provided parameters.

## General tips

There are a few common issues that you may run into, explained here for better reference. Additionally, if you're trying to do something specific with an Aidoku source, chances are one of the sources on this repo does the same thing or something very similar. A good practice is to try searching the repo using relavent keywords.

### Avoid cloning

The most common review comment left on PRs is regarding the excessive use of `clone`. Rust's concept of ownership and borrowing is different from many other common programming languages, and `clone` is a trick to "fix" many compile errors that stem from this. However, since Aidoku sources are compiled to small programs that are interpreted in a constrained environment, particular care has to go into making sure they run as efficiently as possible. Whenever you find yourself using `clone`, it's good to check if you can somehow avoid it. Here are some examples where it can be avoided:
- Typically, function parameters should accept references. For example, a string type would use `&str` instead of `String`.
- When iterating over a `Vec` you can do so with a `&` reference, or by using `iter` instead of `into_iter` for an iterator.
- Cloning is not required when constructing a new `String` using `format!`.

A section on [References and Borrowing](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html) from the Rust book may be helpful.

### Global mutable state

Aidoku sources are guaranteed to be run in a single-threaded environment, so typical practices to handle concurrency like "locking" are not required. While this means you can safely use `static mut`, it is still generally discouraged by the language. Instead, a better solution is to use `RefCell` on your source struct. This allows you to maintain mutable state that is passed to all source functions. For reference, see the [multi.cubari](https://github.com/Aidoku-Community/sources/blob/61c58284aaf55262224b735185f18f8495d8cfba/sources/multi.cubari/src/lib.rs#L24) source.

### Anti-scraping measures

Some websites have particular measures taken to prevent web scraping, which is exactly what Aidoku sources do. However, it is very rare that these measures are completely successful in preventing an Aidoku source from reading the data it needs.

#### JS Obfuscation and Execution

Occasionally, websites use JavaScript execution for fetching page images. Sometimes this has a simple solution, such as reading an array of image urls from an inline script element, and sometimes it is a little more difficult, as in the cases of [multi.mangafire](https://github.com/Aidoku-Community/sources/tree/main/sources/multi.mangafire) and [en.mangago](https://github.com/Aidoku-Community/sources/tree/main/sources/en.mangago).

For MangaFire, a complex, obfuscated script is run to generate a "vrf" value that the page image API endpoint requires to return a valid result. By running the website code through a deobfuscation tool and leveraging the browser debugger, it was possible to determine the process that was used to generate these values, and then replicate it in Rust. However, in cases when the site is frequently updated, this solution is unsustainable.

For Mangago, chapter pages contain a value that can be decrypted using values extracted from obfuscated JavaScript to get page image urls. In addition, some pages require a key to descramble the image. This key is determined by extracting relevant JavaScript code and constructing a small snippet that can be executed by the JavaScript running APIs exposed by the Aidoku source API ([reference](https://github.com/Aidoku-Community/sources/blob/c405c34285247971671389131db92923237e1882/sources/en.mangago/src/helpers.rs#L266)). While this solution is a bit easier to write and maintain if things change, it's also less efficient. JS execution should be saved for situations only where it is strictly necessary, or where it would greatly reduce complexity.

#### Page Scrambling

As mentioned with the Mangago source, some sources encode or scramble the actual page image data. In these cases, you can leverage the `PageImageProcessor` trait. Two examples of this are in [the gigaviewer template](https://github.com/Aidoku-Community/sources/tree/main/templates/gigaviewer), and [multi.mangaplus](https://github.com/Aidoku-Community/sources/tree/main/sources/multi.mangaplus).

Similar to Mangago, gigaviewer sources separate portions of page images into chunks that are "scrambled". The dimensions of these chunks are passed into the `context` parameter, which is a hashmap. Then, the `Canvas` struct is used to draw and copy parts of the image onto a new canvas into the correct positions.

On the other hand, MangaPlus encodes the raw data, which can be decoded simply using an encryption key that the API provides.
