document.addEventListener("DOMContentLoaded", () => {
	const sourceList = document.getElementById("source-list");
	const searchBar = document.getElementById("source-search");
	const languageSelect = document.getElementById("language-select");
	const ratingSelect = document.getElementById("rating-select");

	const langMap = {
		en: "English",
		es: "Spanish",
		fr: "French",
		de: "German",
		ja: "Japanese",
		zh: "Chinese",
		ru: "Russian",
		it: "Italian",
		ko: "Korean",
		pt: "Portuguese",
		id: "Indonesian",
		th: "Thai",
		vi: "Vietnamese",
		tr: "Turkish",
		pl: "Polish",
		ar: "Arabic",
		hi: "Hindi",
	};
	const labelToCode = Object.fromEntries(
		Object.entries(langMap).map(([code, label]) => [label, code]),
	);
	function getLanguageLabel(languages) {
		if (!Array.isArray(languages) || languages.length === 0)
			return "Unknown";
		if (languages.length > 1 || languages.includes("multi"))
			return "Multi-Language";
		return langMap[languages[0]] || languages[0];
	}

	// store filter state, sources, and the source dom elements
	const filterState = {
		query: "",
		language: "",
		rating: "",
	};
	let allSources = [];
	let allSourceElements = [];

	// filter sources and hide filtered out elements
	function filterAndShowSources() {
		let visibleCount = 0;
		const sectionToVisible = new Map();
		for (const { source, li, section } of allSourceElements) {
			// check language
			const languageLabel = getLanguageLabel(source.languages);
			const languageMatch =
				!filterState.language ||
				filterState.language === "" ||
				(filterState.language === "Multi-Language" &&
					languageLabel === "Multi-Language") ||
				languageLabel === filterState.language ||
				(filterState.language !== "" &&
					filterState.language !== "Multi-Language" &&
					languageLabel === "Multi-Language" &&
					Array.isArray(source.languages) &&
					source.languages.includes(
						labelToCode[filterState.language],
					));

			// check content rating
			let ratingMatch = true;
			if (filterState.rating === "safe") {
				ratingMatch =
					!source.contentRating || source.contentRating === 0;
			} else if (filterState.rating === "contains-nsfw") {
				ratingMatch = source.contentRating === 1;
			} else if (filterState.rating === "nsfw") {
				ratingMatch = source.contentRating === 2;
			}

			// check search query (match name, alt name, or url)
			const q = filterState.query.trim().toLowerCase();
			const nameMatch =
				!q || (source.name && source.name.toLowerCase().includes(q));
			const altNames = Array.isArray(source.altNames)
				? source.altNames
				: [];
			const altMatch = !q
				? true
				: altNames.some((alt) => alt && alt.toLowerCase().includes(q));
			const urlMatch = !q
				? true
				: source.baseURL && source.baseURL.toLowerCase().includes(q);
			const queryMatch = nameMatch || altMatch || urlMatch;

			const shouldShow = languageMatch && ratingMatch && queryMatch;
			li.style.display = shouldShow ? "" : "none";
			if (shouldShow) {
				visibleCount++;
				sectionToVisible.set(
					section,
					(sectionToVisible.get(section) || 0) + 1,
				);
			}
		}

		// hide sections if they don't have any visible children
		const allSections = document.querySelectorAll(".language-section");
		allSections.forEach((section) => {
			const visible = sectionToVisible.get(section) || 0;
			section.style.display = visible > 0 ? "" : "none";
		});

		// update total source count
		const totalCountSpan = document.querySelector(".total-count");
		if (totalCountSpan) {
			totalCountSpan.textContent = "Total: " + visibleCount;
		}
	}

	// add event listeners for filter changes
	searchBar.addEventListener("input", (e) => {
		filterState.query = e.target.value;
		filterAndShowSources();
	});
	languageSelect.addEventListener("change", (e) => {
		filterState.language = e.target.value;
		filterAndShowSources();
	});
	ratingSelect.addEventListener("change", (e) => {
		filterState.rating = e.target.value;
		filterAndShowSources();
	});

	// fetch and render initial sources from index.min.json
	fetch("index.min.json")
		.then((response) => {
			if (!response.ok) throw new Error("Failed to load sources");
			return response.json();
		})
		.then((data) => {
			if (
				!data.sources ||
				!Array.isArray(data.sources) ||
				data.sources.length === 0
			) {
				sourceList.textContent = "No sources found.";
				return;
			}

			allSources = data.sources;

			// populate language dropdown
			const languageSet = new Set();
			allSources.forEach((source) => {
				const label = getLanguageLabel(source.languages);
				languageSet.add(label);
			});
			const languageList = Array.from(languageSet).sort((a, b) => {
				if (a === "Multi-Language") return -1;
				if (b === "Multi-Language") return 1;
				return a.localeCompare(b);
			});
			languageList.forEach((lang) => {
				if (
					[...languageSelect.options].some(
						(opt) => opt.value === lang,
					)
				)
					return;
				const opt = document.createElement("option");
				opt.value = lang;
				opt.textContent = lang;
				languageSelect.appendChild(opt);
			});

			// apply language label to sources
			const sources = allSources.map((source) => {
				let languageLabel = getLanguageLabel(source.languages);
				let sortLang =
					languageLabel === "Multi-Language"
						? ""
						: languageLabel.toLowerCase();
				return {
					...source,
					languageLabel,
					sortLang,
				};
			});

			// sort by language and name
			sources.sort((a, b) => {
				if (a.sortLang !== b.sortLang) {
					return a.sortLang.localeCompare(b.sortLang);
				}
				// sort alphabetically if language is the same
				return a.name.localeCompare(b.name);
			});

			// group by language label
			const grouped = {};
			sources.forEach((source) => {
				if (!grouped[source.languageLabel]) {
					grouped[source.languageLabel] = [];
				}
				grouped[source.languageLabel].push(source);
			});

			// render source cells
			sourceList.innerHTML = "";
			allSourceElements = [];
			let isFirst = true;
			Object.keys(grouped).forEach((language) => {
				// create a section for the language group
				const section = document.createElement("section");
				section.className = "language-section";
				section.setAttribute("data-lang-label", language);

				// language header
				const headerRow = document.createElement("div");
				headerRow.className = "language-header-row";
				const langHeader = document.createElement("h2");
				langHeader.textContent = language;

				headerRow.appendChild(langHeader);
				section.appendChild(headerRow);

				// source list
				const ul = document.createElement("ul");
				grouped[language].forEach((source) => {
					const li = document.createElement("li");
					li.className = "source-row";
					li.setAttribute("data-name", source.name.toLowerCase());
					li.setAttribute("data-version", String(source.version));
					li.setAttribute(
						"data-languages",
						(source.languages || []).join(","),
					);
					li.setAttribute(
						"data-content-rating",
						source.contentRating != null
							? String(source.contentRating)
							: "0",
					);
					li.setAttribute("data-lang-label", language);

					const leftDiv = document.createElement("div");
					leftDiv.className = "source-left";

					const infoWrapper = document.createElement("div");
					infoWrapper.className = "source-info-wrapper";

					if (source.iconURL) {
						const icon = document.createElement("img");
						icon.src = source.iconURL;
						icon.alt = source.name + " icon";
						icon.className = "source-icon";
						infoWrapper.appendChild(icon);
					}

					const infoRowStack = document.createElement("div");
					infoRowStack.className = "source-info-row-stack";

					const titleRow = document.createElement("div");
					titleRow.className = "source-title-row";

					const name = document.createElement("span");
					name.textContent = source.name;
					titleRow.appendChild(name);

					const version = document.createElement("span");
					version.textContent = " v" + source.version;
					version.className = "source-version";
					titleRow.appendChild(version);

					let ratingBadge = null;
					if (
						source.contentRating === 1 ||
						source.contentRating === 2
					) {
						ratingBadge = document.createElement("span");
						ratingBadge.className =
							"source-rating-badge " +
							(source.contentRating === 1
								? "source-rating-17"
								: "source-rating-18");
						ratingBadge.textContent =
							source.contentRating === 1 ? "17+" : "18+";
						const tooltip = document.createElement("span");
						tooltip.className = "tooltip";
						tooltip.textContent =
							source.contentRating === 1
								? "This source contains NSFW content"
								: "This source contains primarily NSFW content";
						ratingBadge.appendChild(tooltip);
						ratingBadge.addEventListener("mouseenter", () => {
							tooltip.style.opacity = "1";
							tooltip.style.pointerEvents = "auto";
						});
						ratingBadge.addEventListener("mouseleave", () => {
							tooltip.style.opacity = "0";
							tooltip.style.pointerEvents = "none";
						});
					}
					if (ratingBadge) titleRow.appendChild(ratingBadge);

					infoRowStack.appendChild(titleRow);

					if (source.baseURL) {
						const urlRow = document.createElement("div");
						urlRow.className = "source-url";
						urlRow.textContent = source.baseURL;
						infoRowStack.appendChild(urlRow);
					}

					infoWrapper.appendChild(infoRowStack);
					leftDiv.appendChild(infoWrapper);
					const rightDiv = document.createElement("div");
					rightDiv.className = "source-right";

					const button = document.createElement("a");
					button.href = source.downloadURL;
					button.textContent = "↓";
					button.setAttribute("download", "");
					button.className = "source-download";
					rightDiv.appendChild(button);

					li.appendChild(leftDiv);
					li.appendChild(rightDiv);

					ul.appendChild(li);

					allSourceElements.push({ source, li, section });
				});
				section.appendChild(ul);
				sourceList.appendChild(section);
			});

			// initial filter
			filterAndShowSources();
		})
		.catch((error) => {
			sourceList.textContent = "Error loading sources.";
			console.error(error);
		});
});
