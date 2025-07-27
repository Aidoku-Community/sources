#![expect(clippy::unwrap_used)]

use super::*;
use aidoku::Manga;
use aidoku_test::aidoku_test;

#[aidoku_test]
fn red_1() {
	let manga_page_result: MangaPageResult = serde_json::from_str::<Root>(
		r#"{"code":200,"message":"请求成功","results":{"list":[{"name":"紅羅賓","alias":"紅羅賓,红罗宾","path_word":"hluobin","cover":"https://hi77-overseas.mangafuna.xyz/hluobin/cover/1650990054.jpg.328x422.jpg","author":[{"name":"DC Comics","alias":"DC Comics","path_word":"dccomics"}],"popular":9995,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"紅之花","alias":"紅之花,红之花","path_word":"hongzhihua","cover":"https://hi77-overseas.mangafuna.xyz/hongzhihua/cover/1650973913.jpg.328x422.jpg","author":[{"name":"ハンモック","alias":"ハンモック","path_word":"hammock"}],"popular":382,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"紅椿","alias":"[高橋拡那] 紅椿，红椿","path_word":"kurenaiTsubaki","cover":"https://hi77-overseas.mangafuna.xyz/kurenaiTsubaki/cover/1651449260.jpg.328x422.jpg","author":[{"name":"高橋拡那","alias":null,"path_word":"TakahashiHiromuna"}],"popular":88699,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"妹紅完全詳解","alias":"妹红完全详解","path_word":"meihongwanquanxiangjie","cover":"https://sm.mangafuna.xyz/m/meihongwanquanxiangjie/cover/1730818190.jpg.328x422.jpg","author":[{"name":"つづら","alias":"Tsuzura","path_word":"Tsuzura"}],"popular":2135,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"紅大衣","alias":"红大衣","path_word":"hongdayiimage","cover":"https://sh.mangafuna.xyz/h/hongdayiimage/cover/1713295682.jpg.328x422.jpg","author":[{"name":"image","alias":"image","path_word":"image"}],"popular":2423,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"妖紅綺想","alias":"妖紅綺想,妖红绮想","path_word":"yhqx","cover":"https://hi77-overseas.mangafuna.xyz/yhqx/cover/1650964718.jpg.328x422.jpg","author":[{"name":"未知","alias":"未知","path_word":"weizhi"}],"popular":442,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"紅之魔法","alias":"紅之魔法,红之魔法","path_word":"hongzhimofa","cover":"https://hi77-overseas.mangafuna.xyz/hongzhimofa/cover/1651066543.jpg.328x422.jpg","author":[{"name":"江口","alias":"江口","path_word":"jiangkou2"}],"popular":676,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"青之紅","alias":"青之紅,青之红","path_word":"qingzhihong","cover":"https://hi77-overseas.mangafuna.xyz/qingzhihong/cover/1651041400.jpg.328x422.jpg","author":[{"name":"小匙三杯","alias":"小匙三杯","path_word":"xiaoshisanbei"}],"popular":623,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"紅坦克 （2020）","alias":"红坦克 （2020）,红坦克(2020),紅坦克(2020)","path_word":"hongtankeerlingerling","cover":"https://sh.mangafuna.xyz/h/hongtankeerlingerling/cover/1749405091.jpg.328x422.jpg","author":[{"name":"Marvel Comics","alias":"Marvel Comics","path_word":"marvel"}],"popular":359,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"紅炎之戀","alias":"紅炎之戀,红炎之恋","path_word":"honglianzhiyan","cover":"https://hi77-overseas.mangafuna.xyz/honglianzhiyan/cover/1651387519.jpg.328x422.jpg","author":[{"name":"加藤雄一","alias":"加藤雄一","path_word":"jiatengxiognyi"}],"popular":5601,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"紅之血族","alias":"紅之血族,红之血族","path_word":"hongzhixuezu","cover":"https://hi77-overseas.mangafuna.xyz/hongzhixuezu/cover/1651048819.jpg.328x422.jpg","author":[{"name":"水镜ひより","alias":"水镜ひより","path_word":"hoyori"},{"name":"冰坂透","alias":"冰坂透","path_word":"thoru"}],"popular":564,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"K~紅之記憶","alias":"K 红之记忆,K~紅之記憶,K~红之记忆","path_word":"khzhy","cover":"https://hi77-overseas.mangafuna.xyz/khzhy/cover/1651089691.jpg.328x422.jpg","author":[{"name":"黑荣悠（黒榮ゆい）","alias":"黑荣悠（黒榮ゆい）","path_word":"heiryui"}],"popular":4619,"theme":[],"parodies":[],"females":[],"males":[]}],"total":430,"limit":12,"offset":0}}"#,
	)
	.unwrap()
	.into();
	assert!(manga_page_result.has_next_page);

	let entries = manga_page_result.entries;
	assert_eq!(
		*entries.first().unwrap(),
		Manga {
			key: "hluobin".into(),
			title: "紅羅賓".into(),
			cover: Some("https://hi77-overseas.mangafuna.xyz/hluobin/cover/1650990054.jpg".into()),
			authors: Some(["DC Comics".into()].into()),
			url: Some("https://www.2025copy.com/comic/hluobin".into()),
			..Default::default()
		}
	);

	assert_eq!(
		*entries.last().unwrap(),
		Manga {
			key: "khzhy".into(),
			title: "K~紅之記憶".into(),
			cover: Some("https://hi77-overseas.mangafuna.xyz/khzhy/cover/1651089691.jpg".into()),
			authors: Some(["黑荣悠（黒榮ゆい）".into()].into()),
			url: Some("https://www.2025copy.com/comic/khzhy".into()),
			..Default::default()
		}
	);
}

#[aidoku_test]
fn red_36() {
	let manga_page_result: MangaPageResult = serde_json::from_str::<Root>(
		r#"{"code":200,"message":"请求成功","results":{"list":[{"name":"MURCIÉLAGO-蝙蝠-","alias":"murcielago,蝙蝠杀手,蝙蝠殺手","path_word":"murcielago","cover":"https://hi77-overseas.mangafuna.xyz/murcielago/cover/1673782598.jpg.328x422.jpg","author":[{"name":"よしむらかな","alias":"よしむらかな","path_word":"yoshimurakana"}],"popular":2149320,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"小皇女藥劑師","alias":"小皇女药剂师,小皇女药师","path_word":"xhnyjs","cover":"https://hi77-overseas.mangafuna.xyz/xhnyjs/cover/1691930960.jpeg.328x422.jpg","author":[{"name":"마피","alias":null,"path_word":"mapi"},{"name":"은려원","alias":null,"path_word":"eunlyeowon"}],"popular":145040,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"終極夜魔俠與艾麗卡","alias":"终极夜魔侠与艾丽卡","path_word":"zhongjiyemoxiayuailika","cover":"https://sz.mangafuna.xyz/z/zhongjiyemoxiayuailika/cover/1733735472.jpg.328x422.jpg","author":[{"name":"Marvel Comics","alias":"Marvel Comics","path_word":"marvel"}],"popular":569,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"藍、灰與蝙蝠","alias":"蓝、灰与蝙蝠,异世界蝙蝠侠：蓝、灰与蝙蝠,異世界蝙蝠俠：藍、灰與蝙蝠","path_word":"lanhuiyubianfu","cover":"https://sl.mangafuna.xyz/l/lanhuiyubianfu/cover/1747584936.jpg.328x422.jpg","author":[{"name":"DC Comics","alias":"DC Comics","path_word":"dccomics"}],"popular":123,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"哨兵半機械人","alias":"哨兵半机械人","path_word":"shaobingbaojixieren","cover":"https://ss.mangafuna.xyz/s/shaobingbaojixieren/cover/1747661583.jpg.328x422.jpg","author":[{"name":"Marvel Comics","alias":"Marvel Comics","path_word":"marvel"}],"popular":120,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"失貞的新娘","alias":"失贞的新娘","path_word":"shizhendexinniang","cover":"https://hi77-overseas.mangafuna.xyz/shizhendexinniang/cover/1692371385.jpg.328x422.jpg","author":[{"name":"友麻碧","alias":null,"path_word":"youmabi"},{"name":"藤丸豆ノ介","alias":"藤丸豆ノ介","path_word":"twdzj"}],"popular":144513,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"犬的時間","alias":"犬的時間,犬的时间","path_word":"quandeshijian","cover":"https://sq.mangafuna.xyz/q/quandeshijian/cover/1752079602.jpg.328x422.jpg","author":[{"name":"市川ショウ","alias":"市川ショウ","path_word":"shichuanショウ"}],"popular":299,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"ARANA-蜘蛛","alias":"ARANA-蜘蛛,ARANA-蜘蛛","path_word":"aranazhizhu","cover":"https://hi77-overseas.mangafuna.xyz/aranazhizhu/cover/1651068478.jpg.328x422.jpg","author":[{"name":"アラカワシン","alias":"アラカワシン","path_word":"arakawashin"},{"name":"よしむらかな","alias":"よしむらかな","path_word":"yoshimurakana"}],"popular":21088,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"鄰座的不良少女清水同學染黑了頭髮","alias":"邻座的不良少女清水同学染黑了头发","path_word":"lzdblsnqstxrhltf","cover":"https://sl.mangafuna.xyz/l/lzdblsnqstxrhltf/cover/1724681143.jpg.328x422.jpg","author":[{"name":"底花","alias":null,"path_word":"dihua"},{"name":"真田若楓","alias":"真田若枫","path_word":"sanadawakakaede"}],"popular":173798,"theme":[],"parodies":[],"females":[],"males":[]},{"name":"小美代老師如是說","alias":"小美代老师如是说","path_word":"xiameidailaoshirushishuo","cover":"https://sx.mangafuna.xyz/x/xiameidailaoshirushishuo/cover/1708321573.jpg.328x422.jpg","author":[{"name":"無敵ソーダ","alias":"Muteki Soda","path_word":"mutekisoda"},{"name":"鹿成トクサク","alias":"Kanari Tokusaku","path_word":"KanariTokusaku"}],"popular":2601675,"theme":[],"parodies":[],"females":[],"males":[]}],"total":430,"limit":12,"offset":420}}"#,
	)
	.unwrap()
	.into();
	assert!(!manga_page_result.has_next_page);

	let entries = manga_page_result.entries;
	assert_eq!(entries.len(), 10);

	assert_eq!(
		*entries.first().unwrap(),
		Manga {
			key: "murcielago".into(),
			title: "MURCIÉLAGO-蝙蝠-".into(),
			cover: Some(
				"https://hi77-overseas.mangafuna.xyz/murcielago/cover/1673782598.jpg".into()
			),
			authors: Some(["よしむらかな".into()].into()),
			url: Some("https://www.2025copy.com/comic/murcielago".into()),
			..Default::default()
		}
	);

	assert_eq!(
		*entries.last().unwrap(),
		Manga {
			key: "xiameidailaoshirushishuo".into(),
			title: "小美代老師如是說".into(),
			cover: Some(
				"https://sx.mangafuna.xyz/x/xiameidailaoshirushishuo/cover/1708321573.jpg".into()
			),
			authors: Some(["無敵ソーダ".into(), "鹿成トクサク".into()].into()),
			url: Some("https://www.2025copy.com/comic/xiameidailaoshirushishuo".into()),
			..Default::default()
		}
	);
}
