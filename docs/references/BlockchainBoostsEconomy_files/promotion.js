/* A listener which sends tne data up to the analytics server on user clicking the link. */
const sendMetricsOnClick = function(links, tne) {
    const id = 42;
    const prop = "prop" + id;
    const eVar = "eVar" + id;
    
    const content = "TNE" + tne.type + "-banner-click-" + tne.locale;

    console.log("TNE metrics data", content);
    
    Array.prototype.slice.call(links).forEach(function(link) {
        link.addEventListener("click", function(event) {
            if(typeof s !== "undefined") {
                const data = {};
                data.linkTrackVars = prop + "," + eVar;
                data[prop] = content;
                data[eVar] = content;
                s.tl(true, "o", window.location.pathname.replace(/^.*\/(.*)\..*$/, "$1"), data);
            }
        });
    });
};


//v1
if (window.location.hostname.indexOf("pwc") != -1) {
    //start code

/* Overrides - update spreadsheet! */
const overrides = {en_in:{homepage:{link:"https://www.pwc.in/the-new-equation.html",},sitewide:{link:"https://www.pwc.in/the-new-equation.html"}},es_co:{homepage:{linkText:"¿Qué es La Nueva Ecuación?",learnMore:"Conoce más"},sitewide:{text:"Somos una comunidad que da soluciones, combinando la inteligencia humana, la experiencia y la innovación tecnológica para entregar resultados sostenibles y construir confianza.",cta:"Todo tiene sentido con La Nueva Ecuación.",linkText:"Mira cómo La Nueva Ecuación puede darte soluciones",}},ru_ru:{homepage:{learnMore:"Подробнее"},sitewide:{}},en_be:{homepage:{link:"https://www.pwc.be/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.be/en/the-new-equation.html"}},es_pa:{homepage:{linkText:"¿Qué es La Nueva Ecuación?",learnMore:"Conoce más"},sitewide:{text:"Somos una comunidad de solucionadores, que combina el ingenio humano, la experiencia y la innovación tecnológica para ofrecer resultados sostenibles y generar confianza.",cta:"Todo tiene sentido con La Nueva Ecuación.",linkText:"Descubra cómo La Nueva Ecuación puede resolverlo por usted",}},es_cr:{homepage:{linkText:"¿Qué es La Nueva Ecuación?",learnMore:"Conoce más"},sitewide:{text:"Somos una comunidad de solucionadores, que combina el ingenio humano, la experiencia y la innovación tecnológica para ofrecer resultados sostenibles y generar confianza.",cta:"Todo tiene sentido con La Nueva Ecuación.",linkText:"Descubra cómo La Nueva Ecuación puede resolverlo por usted",}},es_sv:{homepage:{linkText:"¿Qué es La Nueva Ecuación?",learnMore:"Conoce más"},sitewide:{text:"Somos una comunidad de solucionadores, que combina el ingenio humano, la experiencia y la innovación tecnológica para ofrecer resultados sostenibles y generar confianza.",cta:"Todo tiene sentido con La Nueva Ecuación.",linkText:"Descubra cómo La Nueva Ecuación puede resolverlo por usted",}},es_gt:{homepage:{linkText:"¿Qué es La Nueva Ecuación?",learnMore:"Conoce más"},sitewide:{text:"Somos una comunidad de solucionadores, que combina el ingenio humano, la experiencia y la innovación tecnológica para ofrecer resultados sostenibles y generar confianza.",cta:"Todo tiene sentido con La Nueva Ecuación.",linkText:"Descubra cómo La Nueva Ecuación puede resolverlo por usted",}},es_hn:{homepage:{linkText:"¿Qué es La Nueva Ecuación?",learnMore:"Conoce más"},sitewide:{text:"Somos una comunidad de solucionadores, que combina el ingenio humano, la experiencia y la innovación tecnológica para ofrecer resultados sostenibles y generar confianza.",cta:"Todo tiene sentido con La Nueva Ecuación.",linkText:"Descubra cómo La Nueva Ecuación puede resolverlo por usted",}},es_do:{homepage:{linkText:"¿Qué es La Nueva Ecuación?",learnMore:"Conoce más"},sitewide:{text:"Somos una comunidad de solucionadores, que combina el ingenio humano, la experiencia y la innovación tecnológica para ofrecer resultados sostenibles y generar confianza.",cta:"Todo tiene sentido con La Nueva Ecuación.",linkText:"Descubra cómo La Nueva Ecuación puede resolverlo por usted",}},es_ia:{homepage:{linkText:"¿Qué es La Nueva Ecuación?",learnMore:"Conozca más"},sitewide:{text:"Somos una comunidad de solucionadores, que combina el ingenio humano, la experiencia y la innovación tecnológica para ofrecer resultados sostenibles y generar confianza.",cta:"Todo tiene sentido con La Nueva Ecuación.",linkText:"Descubra cómo La Nueva Ecuación puede resolverlo por usted",}},es_ni:{homepage:{linkText:"¿Qué es La Nueva Ecuación?",learnMore:"Conoce más"},sitewide:{text:"Somos una comunidad de solucionadores, que combina el ingenio humano, la experiencia y la innovación tecnológica para ofrecer resultados sostenibles y generar confianza.",cta:"Todo tiene sentido con La Nueva Ecuación.",linkText:"Descubra cómo La Nueva Ecuación puede resolverlo por usted",}},en_cy:{homepage:{link:"https://www.pwc.com.cy/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.com.cy/en/the-new-equation.html"}},es_es:{homepage:{linkText:"¿Qué es La Nueva Ecuación?",link:"https://www.pwc.es/es/the-new-equation.html",learnMore:"Descúbrelo"},sitewide:{text:"Combinamos la creatividad, la experiencia y la innovación tecnológica para ofrecer resultados sostenidos y generar confianza.",cta:"Todo tiene sentido con La Nueva Ecuación.",linkText:"Descubre cómo La Nueva Ecuación puede ayudarte",link:"https://www.pwc.es/es/the-new-equation.html"}},sl_si:{homepage:{linkText:"Kaj je strategija \"Nova enačba\"?",learnMore:"Kliknite za več informacij!"},sitewide:{text:"Smo k rešitvam usmerjena skupnost, ki z združevanjem človeške iznajdljivosti, izkušenj in tehnoloških inovacij zagotavlja trajnostne rezultate in gradi zaupanje.",cta:"Vse poti vodijo k strategiji 'Nova enačba'! ",linkText:"Oglejte si, kako lahko s strategijo  'Nova enačba' pridobite vi!",}},en_ca:{homepage:{link:"https://www.pwc.com/ca/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.com/ca/en/the-new-equation.html"}},es_mx:{homepage:{linkText:"¿Qué es La Nueva Ecuación?",link:"https://www.pwc.com/us/en/the-new-equation.html",learnMore:"Conoce más"},sitewide:{text:"Somos una comunidad enfocada en ofrecer soluciones, combinamos el ingenio humano, la experiencia y la innovación tecnológica, para ofrecer resultados sólidos y generar confianza.",cta:"Todo tiene sentido con La Nueva Ecuación.",linkText:"Ve cómo La Nueva Ecuación puede resolverlo para ti",}},es_pe:{homepage:{linkText:"¿Qué es La Nueva Ecuación?",link:"https://www.pwc.pe/es/la-nueva-ecuacion.html",learnMore:"Descubre más"},sitewide:{text:"Somos una comunidad de solucionadores, combinando ingenio humano, experiencia e innovación tecnológica, para entregar resultados sostenibles y construir confianza.",cta:"Todo se suma a La Nueva Ecuación.",linkText:"Conoce cómo La Nueva Ecuación puede resolverse para ti",link:"https://www.pwc.pe/es/la-nueva-ecuacion.html"}},es_ar:{homepage:{learnMore:"Conocé más"},sitewide:{text:"Somos una comunidad de especialistas, combinamos ingenio, experiencia e innovación tecnológica para entregar resultados sostenibles y construir confianza.",linkText:"See how The New Equation can solve for you",}},es_bo:{homepage:{linkText:"¿Qué es La Nueva Ecuación? ",learnMore:"Ver más"},sitewide:{text:"Somos una comunidad que busca soluciones, combinando ingenio humano, experiencia e innovación tecnológica para entregar resultados sostenibles y generar confianza.",cta:"Todo tiene sentido con La Nueva Ecuación.",linkText:"Vea cómo La Nueva Ecuación puede ayudarle a buscar soluciones",}},vn_vn:{homepage:{linkText:"Chiến dịch \"Hệ cân bằng mới\" là gì?",learnMore:"Tìm hiểu thêm"},sitewide:{text:"Là một cộng đồng chuyên tháo gỡ những vấn đề trong xã hội, chúng tôi kết hợp sự khéo léo và kinh nghiệm của con người cùng sự đột phá trong công nghệ nhằm mang lại kết quả bền vững và xây dựng niềm tin.",cta:"Tất cả đã góp phần tạo nên một 'Hệ cân bằng mới'.",linkText:"\"Hệ cân bằng mới\" giúp giải quyết những vấn đề gì  ",}},sr_rs:{homepage:{linkText:"Kako se novom formulom dolazi do novih rešenja?",learnMore:"Više o tome saznajte ovde"},sitewide:{text:"Mi smo zajednica stručnjaka specijalizovana za rešavanje problema koja zahvaljujući svojoj domišljatosti, iskustvu i novim tehnologijama obezbeđuje trajna rešenja i gradi poverenje.",cta:"Svi ovi elementi čine Novu formulu kojom se dolazi do rešenja.",linkText:"Saznajte kako uz pomoć Nove formule možete doći do rešenja  ",}},hr_hr:{homepage:{linkText:"Što je Nova jednadžba?",learnMore:"Doznajte više."},sitewide:{text:"Mi smo zajednica onih koji nude rješenja, a uz ljudsku domišljatost, iskustvo i tehnološke inovacije postižemo održive rezultate i izgrađujemo povjerenje.",cta:"Sve ovo tvori Novu jednadžbu.",linkText:"Pogledajte kako Vam Nova jednadžba može ponuditi rješenja",}},hu_hu:{homepage:{linkText:"Összeáll a siker új képlete!",},sitewide:{}},uk_ua:{homepage:{linkText:"Що таке Нова Формула успіху?",learnMore:"дізнатися більше"},sitewide:{text:"Ми – команда, яка вирішує найскладніші задачі. Наші нестандартний підхід, досвід та інноваційні технології приносять сталі результати та допомагають будувати довіру.",cta:"Ці доданки становлять Нову Формулу",linkText:"Дізнайтеся, які задачі Вашого бізнесу вирішує Нова Формула",}},nl_nl:{homepage:{linkText:"Wat is The New Equation?",link:"https://www.pwc.nl/nl/the-new-equation.html",learnMore:"Lees meer"},sitewide:{text:"Met een focus op het vinden van oplossingen combineren onze mensen vindingrijkheid, ervaring en technologische innovatie om duurzame resultaten te leveren en vertrouwen op te bouwen.",cta:"Zo komt alles samen. Dat noemen we The New Equation",linkText:"Lees meer over The New Equation",link:"https://www.pwc.nl/nl/the-new-equation.html"}},pt_ao:{homepage:{link:"https://www.pwc.com/ao/pt/the-new-equation.html",},sitewide:{text:"Criamos soluções, combinando a criatividade humana, experiência e inovação tecnológica para entregar resultados sustentados e construir confiança.",linkText:"The New Equation tem a solução que procura. Saiba como",link:"https://www.pwc.com/ao/pt/the-new-equation.html"}},pt_cv:{homepage:{link:"https://www.pwc.com/cv/pt/the-new-equation.html",},sitewide:{text:"Criamos soluções, combinando a criatividade humana, experiência e inovação tecnológica para entregar resultados sustentados e construir confiança.",linkText:"The New Equation tem a solução que procura. Saiba como",link:"https://www.pwc.com/cv/pt/the-new-equation.html"}},fr_ca:{homepage:{linkText:"Qu'est-ce que la Nouvelle équation?",link:"https://www.pwc.com/ca/fr/the-new-equation.html",learnMore:"En savoir plus"},sitewide:{text:"Nous sommes une communauté qui combine résolution de problèmes, ingéniosité humaine, expérience et innovation technologique pour obtenir des résultats durables et inspirer confiance.",cta:"C’est La nouvelle équation.",linkText:"Trouvez la solution dans La nouvelle équation.",link:"https://www.pwc.com/ca/fr/the-new-equation.html"}},ru_kz:{homepage:{linkText:"Что такое Новое Уравнение",learnMore:"подробнее"},sitewide:{text:"Мы - сообщество тех, кто объединяет изобретательность, опыт и технологические инновации для достижения устойчивых результатов и укрепления доверия",cta:"Все это складывается в Новое Уравнение",linkText:"Узнайте, как с Новым Уравнением можно решить вашу проблему",}},zh_tw:{homepage:{linkText:"何謂新方程？",link:"https://www.pwc.tw/zh/the-new-equation.html",learnMore:"了解更多"},sitewide:{text:"我們是一群問題解決者，集結人類智慧、經驗與科技創新，以建立信任及成就永續。",cta:"這就是新方程",linkText:"了解新方程如何為您解決問題",}},de_de:{homepage:{linkText:"The New Equation",link:"https://www.pwc.de/de/the-new-equation.html",learnMore:"Mehr erfahren"},sitewide:{text:"Wir sind Problemlösende, die mit Einfallsreichtum, Erfahrung und technologischer Innovation nachhaltige Ergebnisse erzielen und Vertrauen schaffen.",linkText:"The New Equation – hier mehr erfahren!",link:"https://www.pwc.de/de/the-new-equation.html"}},zh_cn:{homepage:{},sitewide:{text:"解锁新时代新方程，将智慧巧思、实践经验与前沿科技整合为一，助您建立持久信任，开创成就非凡。",cta:"新方程 = 信任 + 成就",linkText:"了解如何解锁新方程",link:"https://www.pwccn.com/zh/the-new-equation.html"}},en_m1:{homepage:{link:"https://www.pwc.com/m1/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.com/m1/en/the-new-equation.html"}},ru_by:{homepage:{learnMore:"Подробнее"},sitewide:{}},en_us:{homepage:{link:"https://www.pwc.com/us/en/the-new-equation.html",},sitewide:{text:"We are a community of solvers combining human ingenuity, experience and technology innovation to deliver sustained outcomes and build trust.",linkText:"Learn more about the new equation",link:"https://www.pwc.com/us/en/the-new-equation.html"}},en_mx:{homepage:{link:"https://www.pwc.com/us/en/the-new-equation.html",},sitewide:{linkText:"Learn more about the new equation",link:"https://www.pwc.com/gx/en/the-new-equation.html"}},es_uy:{homepage:{learnMore:"Conocé más"},sitewide:{text:"Somos una comunidad de especialistas, combinamos ingenio, experiencia e innovación tecnológica para entregar resultados sostenibles y construir confianza.",}},es_cl:{homepage:{learnMore:"Conoce más"},sitewide:{text:"Somos una comunidad de especialistas, combinamos ingenio, experiencia e innovación tecnológica para entregar resultados sostenibles y construir confianza.",}},en_hk:{homepage:{},sitewide:{link:"https://www.pwccn.com/en/the-new-equation.html"}},en_cn:{homepage:{},sitewide:{link:"https://www.pwccn.com/en/the-new-equation.html"}},ja_jp:{homepage:{link:"https://www.pwc.com/jp/ja/the-new-equation.html",},sitewide:{text:"多岐にわたる分野の多様なプロフェッショナルがスクラムを組み、「人」ならではの発想力や経験と「テクノロジー」によるイノベーションを融合しながら、ゆるぎない成果を実現し、信頼を構築します。",linkText:"詳しくはこちら",link:"https://www.pwc.com/jp/ja/the-new-equation.html"}},de_at:{homepage:{},sitewide:{link:"https://www.pwc.at/de/aktuelle-themen/the-new-equation.html"}},en_nz:{homepage:{},sitewide:{link:"https://www.pwc.co.nz/about-us/the-new-equation.html"}},pt_pt:{homepage:{link:"https://www.pwc.pt/pt/the-new-equation.html",},sitewide:{text:"Criamos soluções, combinando a criatividade humana, experiência e inovação tecnológica para entregar resultados sustentados e construir confiança.",linkText:"The New Equation tem a solução que procura. Saiba como",link:"https://www.pwc.pt/pt/the-new-equation.html"}},pt_br:{homepage:{linkText:"O que é A Nova Equação?",link:"https://www.pwc.com.br/pt/a-nova-equacao.html",learnMore:"Saiba mais"},sitewide:{text:"Somos uma comunidade de solvers que combina criatividade humana, experiência e inovação tecnológica para entregar resultados sustentáveis e construir confiança. ",cta:"Essa é A Nova Equação.",linkText:"Veja como A Nova Equação pode ajudar você",link:"https://www.pwc.com.br/a-nova-equacao"}},en_ch:{homepage:{},sitewide:{link:"https://www.pwc.ch/en/the-new-equation.html"}},fr_ch:{homepage:{},sitewide:{link:"https://www.pwc.ch/fr/the-new-equation.html"}},de_ch:{homepage:{},sitewide:{link:"https://www.pwc.ch/de/the-new-equation.html"}},en_nl:{homepage:{},sitewide:{link:"https://www.pwc.nl/en/the-new-equation.html"}},fi_fi:{homepage:{linkText:"Mikä on The New Equation?",link:"https://www.pwc.fi/fi/mika-on-the-new-equation.html",learnMore:"Lue lisää"},sitewide:{text:"Yhdistämme rohkeat ideat monipuoliseen osaamiseemme ja autamme asiakkaitamme luomaan kestäviä ratkaisuja nyt ja tulevaisuudessa.",cta:"It all adds up to the New Equation",linkText:"Lue lisää strategiastamme",link:"https://www.pwc.fi/fi/mika-on-the-new-equation.html"}},en_ng:{homepage:{link:"https://www.pwc.com/ng/en/the-new-equation.html",},sitewide:{cta:"Nigeria-English (en_ng)",link:"https://www.pwc.com/ng/en/the-new-equation.html"}},en_ke:{homepage:{link:"https://www.pwc.com/ke/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.com/ke/en/the-new-equation.html"}},en_za:{homepage:{link:"https://www.pwc.co.za/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.co.za/en/the-new-equation.html"}},en_mu:{homepage:{link:"https://www.pwc.com/ke/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.com/ke/en/the-new-equation.html"}},en_rw:{homepage:{link:"https://www.pwc.com/ke/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.com/ke/en/the-new-equation.html"}},en_tz:{homepage:{link:"https://www.pwc.com/ke/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.com/ke/en/the-new-equation.html"}},en_ug:{homepage:{link:"https://www.pwc.com/ke/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.com/ke/en/the-new-equation.html"}},en_zm:{homepage:{link:"https://www.pwc.com/ke/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.com/ke/en/the-new-equation.html"}},en_gh:{homepage:{link:"https://www.pwc.com/ng/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.com/ng/en/the-new-equation.html"}},en_lr:{homepage:{link:"https://www.pwc.com/ng/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.com/ng/en/the-new-equation.html"}},en_na:{homepage:{link:"https://www.pwc.co.za/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.co.za/en/the-new-equation.html"}},en_ao:{homepage:{link:"https://www.pwc.co.za/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.co.za/en/the-new-equation.html"}},en_bw:{homepage:{link:"https://www.pwc.co.za/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.co.za/en/the-new-equation.html"}},en_mz:{homepage:{link:"https://www.pwc.co.za/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.co.za/en/the-new-equation.html"}},en_sz:{homepage:{link:"https://www.pwc.co.za/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.co.za/en/the-new-equation.html"}},en_zw:{homepage:{link:"https://www.pwc.co.za/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.co.za/en/the-new-equation.html"}},en_mw:{homepage:{link:"https://www.pwc.co.za/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.co.za/en/the-new-equation.html"}},en_15:{homepage:{link:"https://www.pwc.com/m1/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.com/m1/en/the-new-equation.html"}},en_05:{homepage:{link:"https://www.pwc.com/ca/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.com/ca/en/the-new-equation.html"}},fr_05:{homepage:{linkText:"Qu'est-ce que la Nouvelle équation?",link:"https://www.pwc.com/ca/fr/the-new-equation.html",learnMore:"En savoir plus"},sitewide:{text:"Nous sommes une communauté qui combine résolution de problèmes, ingéniosité humaine, expérience et innovation technologique pour obtenir des résultats durables et inspirer confiance.",cta:"C’est La nouvelle équation.",linkText:"Trouvez la solution dans La nouvelle équation.",link:"https://www.pwc.com/ca/fr/the-new-equation.html"}},en_sg:{homepage:{link:"https://www.pwc.com/sg/en/the-new-equation.html",},sitewide:{link:"https://www.pwc.com/sg/en/the-new-equation.html"}},ja_12:{homepage:{link:"https://www.pwc.com/jp/ja/the-new-equation.html",},sitewide:{text:"多岐にわたる分野の多様なプロフェッショナルがスクラムを組み、「人」ならではの発想力や経験と「テクノロジー」によるイノベーションを融合しながら、ゆるぎない成果を実現し、信頼を構築します。",linkText:"詳しくはこちら",link:"https://www.pwc.com/jp/ja/the-new-equation.html"}},en_cb:{homepage:{},sitewide:{link:"https://www.pwc.com/cb/en/the-new-equation.html"}},en_bs:{homepage:{},sitewide:{link:"https://www.pwc.com/bs/en/the-new-equation.html"}},en_bb:{homepage:{},sitewide:{link:"https://www.pwc.com/bb/en/the-new-equation.html"}},en_bm:{homepage:{},sitewide:{link:"https://www.pwc.com/bm/en/the-new-equation.html"}},en_vg:{homepage:{},sitewide:{link:"https://www.pwc.com/vg/en/the-new-equation.html"}},en_ky:{homepage:{},sitewide:{link:"https://www.pwc.com/ky/en/the-new-equation.html"}},en_gy:{homepage:{},sitewide:{link:"https://www.pwc.com/gy/en/the-new-equation.html"}},en_jm:{homepage:{},sitewide:{link:"https://www.pwc.com/jm/en/the-new-equation.html"}},en_tt:{homepage:{},sitewide:{link:"https://www.pwc.com/tt/en/the-new-equation.html"}},en_uk:{homepage:{},sitewide:{linkText:"Find out more about The New Equation",link:"https://www.pwc.co.uk/the-new-equation.html"}},en_au:{homepage:{},sitewide:{link:"https://www.pwc.com.au/the-new-equation"}},};
    const tne = {
        homepage: { linkText: "What is The New Equation?", link: "https://www.pwc.com/gx/en/the-new-equation.html", learnMore: "Learn more"},
        sitewide: { 
            text: "We are a community of solvers combining human ingenuity, experience and technology innovation to deliver sustained outcomes and build trust.", 
            cta: "It all adds up to The New Equation.", 
            linkText: "See how The New Equation can solve for you", 
            link: "https://www.pwc.com/gx/en.html", 
        },
    };


    //determine whether the homepage or sitewide banner is required
    const pageType = digitalData.page.pageInfo.pageType;
    tne.type = pageType.indexOf("home") === -1 ? "sitewide" : "home";

    //determine the locale based on path
    const locale = digitalData.page.content.contentFullPath.split("/");
    const territory = locale[3];
    const language = locale[4].substring(0, 2);
    tne.locale = language + "_" + territory;

    //override page type and locale if present in querystring
    // Not available in IE so have to check here
    if (typeof URLSearchParams === "function") {
        const tneParams = new URLSearchParams(window.location.search);
        if (tneParams.has("tne-type")) {
            tne.type = tneParams.get("tne-type");
        }

        if (tneParams.has("tne-locale")) {
            tne.locale = tneParams.get("tne-locale");
        }
    }

    // Object.assign polyfill https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Object/assign 
    if (typeof Object.assign !== "function") {
        // Must be writable: true, enumerable: false, configurable: true
        Object.defineProperty(Object, "assign", {
            value: function assign(target, varArgs) {
                // .length of function is 2
                "use strict";
                if (target === null || target === undefined) {
                    throw new TypeError("Cannot convert undefined or null to object");
                }

                var to = Object(target);

                for (var index = 1; index < arguments.length; index++) {
                    var nextSource = arguments[index];

                    if (nextSource !== null && nextSource !== undefined) {
                        for (var nextKey in nextSource) {
                            // Avoid bugs when hasOwnProperty is shadowed
                            if (Object.prototype.hasOwnProperty.call(nextSource, nextKey)) {
                                to[nextKey] = nextSource[nextKey];
                            }
                        }
                    }
                }
                return to;
            },
            writable: true,
            configurable: true,
        });
    }

    //update default text based on locale
    if (tne.locale in overrides) {
        // Properties in former are overridden by latter 
        Object.assign(tne.homepage, overrides[tne.locale].homepage);
        Object.assign(tne.sitewide, overrides[tne.locale].sitewide);
    }
    //tne link override for cee
    if(('...c1,al,am,az,by,ba,bg,hr,cz,ee,ge,hu,kz,ks,kg,lt,lv,mk,mn,me,md,pl,ro,ru,tm,rs,sk,si,tj,tm,ua,uz,19').indexOf(digitalData.page.content.contentFullPath.split("/")[3])>-1){
        tne.homepage.link='https://www.pwc.com/c1/en/the-new-equation.html';
        tne.sitewide.link='https://www.pwc.com/c1/en/the-new-equation.html';
    }
    //locale override for pwclegal.lu
    if (digitalData.page.content.contentFullPath.indexOf("www_pwclegal_lu")!= -1){
        tne.homepage.link+='?geo=l1';
        tne.sitewide.link+='?geo=l1';
    }
    if (digitalData.page.content.contentFullPath.indexOf("www_pwclegal_at")!= -1){
        tne.homepage.link+='?geo=a1';
        tne.sitewide.link+='?geo=a1';
    }
    //HTML
    tne.homepage.html = '<div class="tne-feature"><a href="' + tne.homepage.link + '"><span class="title">' + tne.homepage.linkText + '</span><span class="tne-btn">' + tne.homepage.learnMore + '<i class="btn-arrow"></i></span></a><span class="sqh">' + '</span><span class="sqh"></span><span></span><span></span><span></span><span class="sqs"></span><span class="sqs"></span></div>';
    tne.sitewide.html = '<div id="tne-promo"><div class="container"><a href=' + tne.sitewide.link + '><span>' + tne.sitewide.text + '<br /><br />' + tne.sitewide.cta + '</span><br /><span>' + tne.sitewide.linkText + '</span></a></div></div>';
    // CSS
    tne.homepage.style = "#wrapper .tne-feature{background-color:#ffb600;position:relative;border-top:1rem solid #fff}#wrapper .tne-feature a{text-decoration:none;font:normal normal 2rem/2.5rem 'PwC ITC Charter',Georgia,serif;color:#2d2d2d;display:flex;align-items:center;flex-wrap:wrap}#wrapper .tne-feature span.title{display:flex;align-items:center;position:relative;padding:1rem 1rem 1rem 90px}#wrapper .tne-feature span.title:before{content:'';display:block;width:32px;height:32px;background:#db536a;position:absolute;top:0;left:32px;box-shadow:-32px 32px 0 0 #2d2d2d}#wrapper .tne-feature span.title:after{content:'';display:block;width:16px;height:16px;background:#2d2d2d;position:absolute;top:80px;left:48px}#wrapper .tne-feature span.tne-btn{font:bold normal 1rem/1.5rem 'PwC Helvetica Neue',Arial,sans-serif;display:inline-block;padding:.75rem 2rem .75rem 1.5rem;margin:0 0 1rem 90px;border:1px solid #2d2d2d;white-space:nowrap;transition:all .6s ease;position:relative}#wrapper .tne-feature span.tne-btn i{border:solid #2d2d2d;border-width:0 2px 2px 0;display:inline-block;padding:3px;width:8px;height:8px;margin-left:7px;margin-bottom:1px;transform:rotate(-45deg);transition:border-color .6s ease}#wrapper .tne-feature a:hover span.tne-btn{background-color:#e0301e;border-color:#e0301e;color:#fff}#wrapper .tne-feature a:hover span.tne-btn i{border-color:#fff}@media (min-width:481px){#wrapper .tne-feature a{font-size:2.5rem;line-height:3.25rem;padding-right:330px}#wrapper .tne-feature span.title{height:150px;margin:0}#wrapper .tne-feature>span{display:block;width:32px;height:32px;background:#2d2d2d;position:absolute;top:0;right:55px}#wrapper .tne-feature>span:nth-child(2){transition:transform .5s ease-in-out}#wrapper .tne-feature>span:nth-child(3){background:#d04a02;top:32px;right:87px;transition:transform .8s ease-in-out}#wrapper .tne-feature>span:nth-child(4){background:#e0301e;top:130px;right:160px;width:20px;height:20px}#wrapper .tne-feature>span:nth-child(5){background:#db536a;top:55px;right:25px;width:28px;height:28px}#wrapper .tne-feature>span:nth-child(6){background:#db536a;top:80px;right:290px;width:14px;height:14px}#wrapper .tne-feature>span:nth-child(7){background:#d04a02;top:50px;right:230px;transition:transform .4s ease-in-out}#wrapper .tne-feature>span:nth-child(8){background:#fff;top:82px;right:198px;transition:transform .6s ease-in-out}#wrapper .tne-feature .sqs{transform:scale(0)}#wrapper .tne-feature .sqh{transform:scale(1)}#wrapper .tne-feature:hover .sqs{transform:scale(1)}#wrapper .tne-feature:hover .sqh{transform:scale(0)}}@media (min-width:1200px){#wrapper .tne-feature span.title{margin-left:calc((100vw - 1170px)/ 2)}#wrapper .tne-feature a{flex-wrap:nowrap}#wrapper .tne-feature span.title{padding:0 1rem 1rem 90px}#wrapper .tne-feature span.tne-btn{margin-left:50px}}@media (min-width:1400px){#wrapper .tne-feature span.title{margin-left:calc((100vw - 1370px)/ 2)}}"
    tne.sitewide.style = "#tne-promo{position:relative;background:#fff}#tne-promo a{display:block;text-decoration:none;color:#2d2d2d;border-top:3px solid #2d2d2d;padding:6.875rem 20px 280px 0;position:relative;background:url(/content/dam/pwc/gx/en/the-new-equation/tne-sitepromo.jpg) center bottom no-repeat;background-size:contain}#tne-promo em{font-style:normal;color:#ffb600}#tne-promo a span:first-child{display:block;font:normal normal 2rem/2.375rem 'PwC ITC Charter',Georgia,serif;margin:0 -20px 1rem 0;position:relative}#tne-promo a span:last-child{display:inline;font:normal normal 1.125rem/1.5rem 'PwC Helvetica Neue',Arial,sans-serif;position:relative;padding-bottom:.1rem;transition:all .5s ease;border-bottom:1px solid transparent}#tne-promo a:hover span:last-child{color:#e0301e;border-color:#e0301e}#tne-promo a span:last-child:after{content:'';display:inline;width:21px;height:9px;background-image:url(" + '"' + "data:image/svg+xml,%3Csvg width='21' height='9' viewBox='0 0 21 9' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M18.1062 4.70831L15.8687 7.08805L16.75 8.02271L20.5 4.04543L16.75 0.0681593L15.8687 1.00282L18.1062 3.38255L0.5 3.38255L0.5 4.70831L18.1062 4.70831Z' fill='%23E0301E'/%3E%3C/svg%3E%0A" + '"' + ");position:absolute;bottom:.35rem;right:-1.75rem}#tne-promo .container{position:relative}#tne-promo .container:before{content:'';display:block;width:40px;height:40px;background:#db536a;position:absolute;left:15px;top:30px}#tne-promo .container:after{content:'';display:block;width:15px;height:15px;background:#ffb600;position:absolute;left:55px;top:15px}#tne-promo a span:first-child:before{content:'';display:block;width:15px;height:15px;background:#e0301e;position:absolute;right:20px;top:-40px}@media (min-width:481px){#tne-promo a{padding-bottom:78%}}@media (min-width:992px){#tne-promo a{padding-bottom:10%;background-position:right center;background-size:55%}#tne-promo span{max-width:40%}}"

    if(tne.locale === "en_uk"){
        tne.sitewide.style = tne.sitewide.style.replace("tne-sitepromo.jpg","uk-sitepromo.jpg");
        console.log('TNE en_uk overrides in place');
    };
    if(tne.locale === "vn_vn") {
        tne.homepage.style = tne.homepage.style.replace("'PwC ITC Charter',Georgia,serif","'PwC Helvetica Neue',Arial,Sans-Serif");
        tne.sitewide.style = tne.sitewide.style.replace("'PwC ITC Charter',Georgia,serif","'PwC Helvetica Neue',Arial,Sans-Serif");
        console.log('TNE vn_vn font overrides in place');
    };
    
    /* Exclusions - be careful - update spreadsheet! */
	const tneexcludes = ["/content/pwc/uk/en/who-we-are/annual-report","/content/pwc/ca/fr.html","/content/pwc/ca/en.html","/content/pwc/m1/en.html","/content/pwc/gx/en.html","the-new-equation","/content/pwc/ru/ru/picpe/sveden","/content/pwc/us/en","/content/pwc/uk/en/services/business-restructuring","/content/pwc/jp/ja/about-us/member/pwc-foundation","/content/pwc/jp/en/about/member/pwc-foundation","/content/pwc/mx/es.html","/content/pwc/sg/en.html","/content/pwc/jp/ja.html","/content/pwc/in/en.html","/content/pwc/br/pt.html","/content/pwc/de/de.html","/content/pwc/12/ja.html","/content/pwc/ch/de.html","/content/pwc/ch/fr.html","/content/pwc/ch/en.html","/content/pwc/cb/en.html","/content/pwc/bs/en.html","/content/pwc/bb/en.html","/content/pwc/bm/en.html","/content/pwc/vg/en.html","/content/pwc/ky/en.html","/content/pwc/gy/en.html","/content/pwc/jm/en.html","/content/pwc/tt/en.html","/content/pwc/nz/en.html","/content/pwc/es/es.html","/content/pwc/at/de.html","/content/pwc/uk/en.html","/content/pwc/tw/zh.html","/content/pwc/nl/nl.html","/content/pwc/nl/en.html","/content/pwc/nl/nl/carriere","/content/pwc/nl/en/careers","/content/pwc/be/en.html","/content/pwc/fi/fi.html","/content/pwc/cn/zh.html","/content/pwc/cn/en.html","/content/pwc/hk/en.html","/content/pwc/au/en.html"];
    
    /* Inclusions overruling tneexcludes - update spreadsheet! */
    const forceInclusionList = ["/content/pwc/uk/en/the-new-equation/"]; /* SCTASK10863042 - To enable on all child pages of tne under uk */
    
    /* Set tneexclude to false if page is force included */
    const forceInclude = forceInclusionList.filter(function(forceDisplayPath) {
        return digitalData.page.content.contentFullPath.indexOf(forceDisplayPath) >= 0;
    }).length ? true : false;
    
    /* Set tneexclude to true if the current page should be excluded from displaying the "promotion" */
    const tneexclude = !forceInclude && tneexcludes.filter(function(tneexclude) {
        return digitalData.page.content.contentFullPath.indexOf(tneexclude) >= 0;
    }).length ? true : false;

    if(!tneexclude) {
        $(document).ready(function () {
            // Inject CSS & HTML
            if(tne.type === "home") {
                const style = document.createElement("style");
                style.innerHTML = tne.homepage.style;
                document.head.appendChild(style);

                $(tne.homepage.html).insertAfter("#promotion");
            }
            else {
                const style = document.createElement("style");
                style.innerHTML = tne.sitewide.style;
                document.head.appendChild(style);

                $(tne.sitewide.html).insertBefore($(".newPageFooter").first());
            }

            sendMetricsOnClick(document.querySelectorAll("#tne-promo a, .tne-feature a"), tne); /* Attach an event listener to the banner to send analytics data on click */
            console.log('TNE loaded and executed.'); 
            console.log(overrides);
            console.log(tne);
        });
    }
    else {
        console.log('TNE loaded and not run due to path.');
    };
    //end code
}
else {
    console.log('TNE loaded and not run due to domain.');
};
