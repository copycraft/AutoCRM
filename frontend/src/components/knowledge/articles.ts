export interface KnowledgeArticle {
  slug: string;
  group: string;
  title: string;
  summary: string;
  keywords: string[];
  body: { heading?: string; paragraphs: string[] }[];
  links: { label: string; href: string }[];
}

export const KNOWLEDGE_GROUPS = [
  'Kezdés',
  'Értékesítés',
  'Műhely',
  'Számlázás',
  'Levelezés',
  'HR',
  'Mindennapi használat',
] as const;

export const KNOWLEDGE_ARTICLES: KnowledgeArticle[] = [
  {
    slug: 'iranyitopult',
    group: 'Kezdés',
    title: 'Irányítópult: mivel kezdd a napot?',
    summary: 'Munkában lévő autók, figyelmet igénylő tételek, ajánlati cső, saját teendők.',
    keywords: ['dashboard', 'irányítópult', 'kezdés', 'áttekintés', 'teendő'],
    body: [
      {
        paragraphs: [
          'Az Irányítópult a napi kiindulópont: felül a számok (Munkában, Nyitott akadály, Megrekedt, Lejáró ajánlat), alatta a részletek.',
          'A „Figyelmet igényel" sávban a nyitott akadályok, a megrekedt munkák és a 7 napon belül lejáró árajánlatok vannak. Amit itt látsz, azzal érdemes aznap foglalkozni.',
        ],
      },
      {
        heading: 'Mit hol találsz?',
        paragraphs: [
          'Munkában lévő autók: minden nyitott megrendelés, fázissal és mióta áll ott. Kattintásra nyílik a megrendelés.',
          'Ajánlati cső: a nyitott leadek fázisonként. Ha üres, nincs függő ajánlat.',
          'Teendőim: a saját nyitott teendőid, határidővel. Lejárt külön jelölve.',
          'Átvehető: elkészült, átadásra váró autók. Ez megy ki a műhelykijelzőre is.',
        ],
      },
    ],
    links: [
      { label: 'Megrendelések', href: '/hu/orders' },
      { label: 'Leadek', href: '/hu/leads' },
    ],
  },
  {
    slug: 'partnerek',
    group: 'Értékesítés',
    title: 'Partnerek: cégek és magánvevők',
    summary: 'Új partner felvétele, kapcsolattartók, archiválás, folyószámla.',
    keywords: ['partner', 'cég', 'vevő', 'kapcsolattartó', 'adószám', 'folyószámla'],
    body: [
      {
        paragraphs: [
          'Két lista van: Üzleti partnerek (cégek) és Fogyasztók (magánszemélyek). Új partnert az „Új" gombbal veszel fel; az adószám formátuma 12345678-1-23.',
          'Egy partnerhez több kapcsolattartó tartozhat (név, beosztás, e-mail, telefon). Partner váltásakor a kapcsolattartó törlődik, ha nem választasz újat — erre a felület figyelmeztet.',
        ],
      },
      {
        heading: 'Archiválás és folyószámla',
        paragraphs: [
          'Az archivált partner megmarad az előzményekben, de új választásokban nem jelenik meg. Ugyanez igaz a kapcsolattartóra.',
          'A partner adatlapján a folyószámla (Folyószámla gomb) nyomtatható kimutatás: időszak + nyitott számlák, befizetések, pénznemenkénti egyenleg.',
          'A számlázási nyelv (magyar/német/angol) a partner adatlapján állítható — a számla és a díjbekérő ezen a nyelven készül.',
        ],
      },
    ],
    links: [
      { label: 'Üzleti partnerek', href: '/hu/partners/business' },
      { label: 'Számlázó', href: '/hu/invoices' },
    ],
  },
  {
    slug: 'leadek',
    group: 'Értékesítés',
    title: 'Leadek: érdeklődéstől a megrendelésig',
    summary: 'Lead felvétele, fázisváltás, átalakítás megrendeléssé, elvesztés oka.',
    keywords: ['lead', 'érdeklődő', 'fázis', 'átalakítás', 'megnyert', 'elveszett'],
    body: [
      {
        paragraphs: [
          'A lead egy érdeklődés: címe, partnere, kapcsolattartója, felelőse, forrása és értéke van. A lista kereshető, szűrhető (fázis, felelős, címke, forrás), mentett nézetekkel és exporttal.',
          'Fázisváltás: a lead adatlapján a Fázisváltás gombbal. Hátra- vagy újranyitáskor indoklás kötelező. A „Megnyert" fázis csak átalakítással érhető el — kézzel nem kapcsolható.',
        ],
      },
      {
        heading: 'Átalakítás és lezárás',
        paragraphs: [
          'Ha az ügyfél megrendeli: „Átalakítás megrendeléssé". Ehhez partner kell a leaden. A megrendelés pénzneme a partner alapértelmezettjével töltődik.',
          'Ha nem lesz belőle munka: a kilépő fázis (Elveszett) választásakor elvesztés okát kell megadni. Az okok a Leadek → Címkék kezelése oldalon szerkeszthetők.',
          'Duplikálás: meglévő leadből az „Új lead / Duplikálás" indul — a piszkozat nem öröklődik, mindent ellenőrizz.',
        ],
      },
    ],
    links: [
      { label: 'Leadek', href: '/hu/leads' },
      { label: 'Árajánlat küldése', href: '/hu/tudastar?cikk=arajanlat' },
    ],
  },
  {
    slug: 'arajanlat',
    group: 'Értékesítés',
    title: 'Árajánlat és utánkövető levelek',
    summary: 'Árajánlat küldése PDF-melléklettel, automatikus utánkövetés 1-2-4 hét múlva.',
    keywords: ['árajánlat', 'ajánlat', 'utánkövetés', 'follow-up', 'lejárat', 'érvényesség'],
    body: [
      {
        paragraphs: [
          'Az árajánlatot a lead adatlapján az „Árajánlat küldése" gombbal küldöd a lead kapcsolattartójának. A lead PDF-dokumentumai automatikusan mellékletként mennek.',
          'A tárgy alapértelmezettje „Árajánlatunk: {cím}", a fejléc-szöveg üresen hagyva a lead adataiból épül fel. Markdownban is írhatsz, élő előnézettel.',
        ],
      },
      {
        heading: 'Mi történik küldés után?',
        paragraphs: [
          'A küldés beütemezi az utánkövető leveleket (alapból 1 hét, 2 hét, 1 hónap múlva). A sorozat a Sablonok → Értékesítés alatt szerkeszthető.',
          'Az utánkövetés leáll, ha a lead megnyert/elveszett/átalakított lesz, vagy ha az ügyfél válaszol (a postaláda-olvasó ezt észleli).',
          'Ha az ajánlat érvényessége 7 napon belül lejár, a felelős (felelős nélküli leadnél minden admin és irodai dolgozó) egyszer értesítést kap érvényességi dátumonként.',
        ],
      },
    ],
    links: [
      { label: 'Leadek', href: '/hu/leads' },
      { label: 'Sablonok', href: '/hu/templates' },
    ],
  },
  {
    slug: 'cimkek-forrasok',
    group: 'Értékesítés',
    title: 'Címkék, források, honnan jött az ügyfél',
    summary: 'Piaconkénti lead-címkék, domain-igénylés, forrás és csatorna.',
    keywords: ['címke', 'tag', 'forrás', 'utm', 'kampány', 'domain', 'piac'],
    body: [
      {
        paragraphs: [
          'A címkék piaconkénti listák (pl. Magyar, Román, Német): Leadek → Címkék kezelése. Itt szerkeszthetők a források és az elvesztés okai is.',
          'Egy címke weboldal-domaineket igényelhet ki: az olyan érkező lead, amelynek oldala, hivatkozója vagy UTM-forrása a domain alá esik (aldomainekkel), automatikusan megkapja a címkét. Egy domain egyszerre csak egy élő címkéhez tartozhat — a felület jelzi, ha foglalt.',
        ],
      },
      {
        heading: 'Forrás és csatorna',
        paragraphs: [
          'A leaden a forrás (pl. web, telefon, vásár) és részletei kézzel is megadhatók. A honlapról érkezőknél az UTM-paraméterekből a rendszer csatornát számol (fizetett, organikus, közösségi, e-mail, hivatkozó, közvetlen).',
          'A Jelentésekben leadek és megnyerések láthatók csatornánként, kampányonként és céloldalanként, plusz a webes konverzió elvesztési okokkal.',
        ],
      },
    ],
    links: [
      { label: 'Címkék kezelése', href: '/hu/leads/tags' },
      { label: 'Jelentések', href: '/hu/reports' },
    ],
  },
  {
    slug: 'megrendelesek',
    group: 'Műhely',
    title: 'Megrendelések és fázisok',
    summary: 'Munka felvétele, tételek, specifikáció, fázisváltás és fotókapuk.',
    keywords: ['megrendelés', 'munka', 'fázis', 'fotókapu', 'határidő', 'tétel', 'specifikáció'],
    body: [
      {
        paragraphs: [
          'Új megrendelés az „Új megrendelés" gombbal: partner, cím, projekttípus, felelős, határidő, tételek (leírás, mennyiség, egységár). A specifikáció (hűtési/fűtési) a létrehozással együtt utazik — nincs árva specifikáció.',
          'A megrendelés száma ÉÉÉÉ-SORSZ formátumú (pl. 2026-0042). Amíg tételei vannak, a pénznem nem módosítható.',
        ],
      },
      {
        heading: 'Fázisváltás szabályai',
        paragraphs: [
          'Előre több fázist is lehet ugrani, de a kihagyott fázisok képfeltételei (fotókapuk) is érvényesek — MEO-hoz például kell elkészültfotó.',
          'Hátra vagy újranyitáskor indoklás kötelező. Az elveszett/lemondott fázisok mindig elérhetők. Az átvétel elhagyásához a km-óra kell az átvételi lapon.',
          'A fázissáv mutatja az aktuális fázist, alatta az előzmény. A „Traveller" csík a munka útját összegzi.',
        ],
      },
    ],
    links: [
      { label: 'Megrendelések', href: '/hu/orders' },
      { label: 'Átvételi lap', href: '/hu/tudastar?cikk=atveteli-lap' },
    ],
  },
  {
    slug: 'atveteli-lap',
    group: 'Műhely',
    title: 'Átvételi lap, járművek, alvázszám',
    summary: 'Km-óra, üzemanyag, kulcsok, értéktárgy; több jármű egy munkán; VIN-kiolvasás.',
    keywords: ['átvételi lap', 'km', 'jármű', 'rendszám', 'vin', 'alvázszám', 'üzemanyag'],
    body: [
      {
        paragraphs: [
          'Az átvételi lap a megrendelés adatlapján rögzíthető: km-óra (kötelező az átvétel lezárásához), állapot, üzemanyagszint, kulcsok száma, értéktárgy (nincs / van, tételesen nem rögzítve / leírás).',
          'Egy munkához több jármű is tartozhat. A rendszám-keresés elgépelést tűr (szóköz, kötőjel, kis-nagybetű nem számít). Ha az autó már járt itt, a felület figyelmeztet és mutatja a korábbi munkát — folytatás előtt nyisd meg.',
        ],
      },
      {
        heading: 'Alvázszám (VIN)',
        paragraphs: [
          'A 17 karakteres VIN-ből a rendszer gyártót és modellévet olvas (ellenőrző számjegyet is néz — ha nem stimmel, valószínűleg elírás).',
          'A márka és típus egy gombbal átvehető a VIN-ből. Az online NHTSA-lekérdezés csak akkor él, ha be van kapcsolva; egyébként minden helyben történik.',
          'Hűtős munkán a hűtőgép gyári száma külön mezőben rögzíthető.',
        ],
      },
    ],
    links: [
      { label: 'Megrendelések', href: '/hu/orders' },
      { label: 'Vizsgálat', href: '/hu/tudastar?cikk=vizsgalat' },
    ],
  },
  {
    slug: 'vizsgalat',
    group: 'Műhely',
    title: 'Átvétel–kiadás vizsgálat (körbejárás)',
    summary: 'Telefonon végigvezetett körbejárás, zónák, sérülések, aláírások.',
    keywords: ['vizsgálat', 'átvétel', 'kiadás', 'körbejárás', 'sérülés', 'aláírás', 'jegyzőkönyv'],
    body: [
      {
        paragraphs: [
          'A vizsgálat telefonon készül, végigvezetett körbejárással; az irodai gépen csak az eredménye látszik. Egyszerre egy nyitott jegyzőkönyv lehet munkánként.',
          'Átvételkor a sérülések „már megvolt" állapottal rögzülnek. Kiadáskor a rendszer összeveti a két körbejárást: minden új sérülésnél dönteni kell (új sérülés / már megvolt / nem sérülés), csak utána zárható le.',
        ],
      },
      {
        heading: 'Lezárás feltételei',
        paragraphs: [
          'A kötelezőnek jelölt fotók nélkül a jegyzőkönyv nem zárható le (a lista a Beállításokban szerkeszthető, járműtípusonként, átvételre és kiadásra külön).',
          'A lezáráshoz mindkét aláírás (átadó + sofőr/ügyfél) kell. Az aláírt jegyzőkönyv zárolt: utólag csak időbélyegzett megjegyzés fűzhető hozzá.',
          'Kiadáshoz aláírt átvételi jegyzőkönyv kell. A két jegyzőkönyv összehasonlítása a megrendelés adatlapján nézhető meg, km-különbséggel.',
        ],
      },
    ],
    links: [
      { label: 'Megrendelések', href: '/hu/orders' },
      { label: 'Fotók', href: '/hu/tudastar?cikk=fotok' },
    ],
  },
  {
    slug: 'fotok',
    group: 'Műhely',
    title: 'Fotók és dokumentumok',
    summary: 'Feltöltés, kategóriák, védett bevételi képek, megjegyzések, ZIP-letöltés.',
    keywords: ['fotó', 'kép', 'dokumentum', 'feltöltés', 'bevétel', 'védett', 'zip'],
    body: [
      {
        paragraphs: [
          'Képek kategóriái: Bevétel, Gyártás, Átadás/MEO, Marketing. A dokumentumok típusai: terv, CAD, egyéb, számlához tartozó bizonylat, videó.',
          'Feltöltés: fájlok kiválasztása vagy húzása. Ha ugyanaz a fájl már fent van, a rendszer jelzi — nem tölti fel kétszer.',
        ],
      },
      {
        heading: 'Védett képek és egyebek',
        paragraphs: [
          'A bevételi (bizonyítási célú) képek védettek: nem törölhetők. Az eredeti megtekintése külön jogosultsághoz kötött.',
          'A képekhez felirat és rajzos megjegyzés fűzhető, van összehasonlító (előtte–utána) nézet, a megrendelés képei egyben ZIP-ben letölthetők.',
          'Beszállítói számlánál a PDF-et vagy fotót csak dobd be: a fájl csatolva tárolódik, az adatokat (összeg, dátum) kézzel rögzíted.',
        ],
      },
    ],
    links: [
      { label: 'Megrendelések', href: '/hu/orders' },
      { label: 'Bejövő számlák', href: '/hu/incoming-invoices' },
    ],
  },
  {
    slug: 'akadalyok',
    group: 'Műhely',
    title: 'Akadályok, teendők, megjegyzések',
    summary: 'Akadály felvétele a munkán, emlékeztetők, saját teendők, idővonal.',
    keywords: ['akadály', 'blocker', 'emlékeztető', 'teendő', 'feladat', 'megjegyzés', 'idővonal'],
    body: [
      {
        paragraphs: [
          'Akadályt a megrendelés adatlapján veszel fel: mi akadályoz, ki a felelős (partner + e-mail), határidő. Megoldáskor megoldási megjegyzéssel zárod; újranyitható.',
          'Ha az emlékeztetés be van kapcsolva, a rendszer óránként küld emlékeztetőt a felelősnek, adott szám után eszkalálva. A ritmus a Beállításokban állítható.',
        ],
      },
      {
        heading: 'Teendők és előzmények',
        paragraphs: [
          'Teendő bármelyik rekordhoz (megrendelés, lead, partner, munkatárs) felvehető, határidővel. A sajátjaidat az Irányítópult és a Feladatok mutatják; készre pipálható, törlés visszavonható.',
          'Minden rekordnak egy idővonala van: mezőváltozások, fázisok, fájlok, kiküldött és beérkezett levelek, MiniCRM-előzmények. A megjegyzéseknél munkatárs említhető.',
        ],
      },
    ],
    links: [
      { label: 'Megrendelések', href: '/hu/orders' },
      { label: 'Irányítópult', href: '/hu' },
    ],
  },
  {
    slug: 'udvar-kijelzo',
    group: 'Műhely',
    title: 'Udvar és műhelykijelző',
    summary: 'Udvari térkép, járműmozgatás, túlterhelés-jelzés; a TV-s átvételi tábla.',
    keywords: ['udvar', 'térkép', 'mozgatás', 'kijelző', 'tv', 'átvehető', 'parkoló'],
    body: [
      {
        paragraphs: [
          'Az Udvar oldalon 2D-s rajzon (és 3D-nézetben) látod a járműveket. Húzással mozgathatók; a mozgás naplózva van, korábbi állapot visszaállítható (revíziók).',
          'A helyeknek kapacitása van: túlterhelést a felület jelez, de nem tilt — a döntést rád bízza. A helyek listája (öböl stb.) szerkeszthető.',
          'Egy-egy megrendelés adatlapján a jármű udvari helye külön is beállítható, legördülővel.',
        ],
      },
      {
        heading: 'Műhelykijelző',
        paragraphs: [
          'A Kijelző (Board) a műhely-tévére való: az átvehető és a készülő autók listája, automatikusan frissülve.',
          'Külön oldal, bejelentkezve érhető el — tedd ki a tévé böngészőjében.',
        ],
      },
    ],
    links: [
      { label: 'Udvar', href: '/hu/yard' },
      { label: 'Kijelző', href: '/hu/board' },
    ],
  },
  {
    slug: 'szamlazo',
    group: 'Számlázás',
    title: 'Számlázó: számla, sztornó, díjbekérő',
    summary: 'Számla kiállítása NAV-bejelentéssel, sztornó, technikai érvénytelenítés, fizetve jelölés.',
    keywords: ['számla', 'sztornó', 'díjbekérő', 'nav', 'fizetve', 'proforma', 'pdf'],
    body: [
      {
        paragraphs: [
          'Számla a megrendelés adatlapján vagy a Számlázó oldalon állítható ki („Számla kiállítása"). A megrendelés tételeiből készül, és a rendszer bejelenti a NAV Online Számla felé — a válasz néhány másodperc múlva érkezik, a felület addig „Bejelentés folyamatban" állapotot mutat.',
          'Egy munkához egyszerre egy élő számla tartozhat. Ha javítani kell: sztornó számla készül (saját sorszámmal, NAV-hoz bejelentve), sosem szerkesztjük a kiállítottat.',
        ],
      },
      {
        heading: 'Fizetés, díjbekérő, érvénytelenítés',
        paragraphs: [
          'Készpénzes számla kiállításkor fizetett; átutalásost az iroda jelöl fizetettnek (és vissza is vonhatja). A PDF a számla adatlapján tölthető le.',
          'Díjbekérő (proforma): fizetési felszólítás, nem adóügyi bizonylat — áfalevonásra nem jogosít, NAV-hoz nem jelentjük. Számla előtt is küldhető.',
          'Technikai érvénytelenítés: azt jelenti, hogy maga az adatszolgáltatás volt hibás (nem a számla). Csak bejelentett számlánál, adminisztrátorral, és a NAV-portálon még jóvá kell hagyni. NAV-kódot (pl. Hibás adat) és indoklást kér.',
          'Lejárt átutalásos számlákra a rendszer fizetési emlékeztetőt küld (alapból a határidő után 3 és 14 nappal) — számlánként kikapcsolható.',
        ],
      },
    ],
    links: [
      { label: 'Számlázó', href: '/hu/invoices' },
      { label: 'Megrendelések', href: '/hu/orders' },
    ],
  },
  {
    slug: 'bejovo-szamlak',
    group: 'Számlázás',
    title: 'Bejövő (szállítói) számlák',
    summary: 'PDF/fotó bedobása, adatok rögzítése, listák és export.',
    keywords: ['bejövő', 'szállító', 'beszerzés', 'költség', 'pdf'],
    body: [
      {
        paragraphs: [
          'A Bejövő számlák oldalon a szállítói számlák vannak: a PDF-et vagy fotót az oldalra húzod, majd kitöltöd az adatokat (szállító, összeg, dátum, stb.).',
          'Amelyik listára kerül, az az adatokból következik (pl. nyitott/lejárt/rendezett). Tömegesen is kezelhetők, exportálhatók, mindegyiknek saját idővonala van.',
        ],
      },
    ],
    links: [{ label: 'Bejövő számlák', href: '/hu/incoming-invoices' }],
  },
  {
    slug: 'emailek',
    group: 'Levelezés',
    title: 'E-mailek és sablonok',
    summary: 'Kézi levélírás, automatikus levelek, tiltólista, sablonok és változók.',
    keywords: ['e-mail', 'levél', 'sablon', 'változó', 'tiltólista', 'melléklet', 'markdown'],
    body: [
      {
        paragraphs: [
          'Új levél az E-mailek → Új e-mail: közvetlen levél egy ügyfélnek, vagy hírlevél mindenkinek. Markdownban is írható, élő előnézettel; a megrendelés/lead dokumentumai csatolhatók vagy képként beágyazhatók.',
          'A kimenő lista mutatja az állapotot: várakozik, küldés alatt, elküldve, sikertelen, visszavonva, átnézésre vár. A várakozó visszavonható, a sikertelen újraküldhető.',
        ],
      },
      {
        heading: 'Automatikus levelek és sablonok',
        paragraphs: [
          'Az automatikus levelek (emlékeztetők, utánkövetők, visszaigazolások, riasztások) csak akkor mennek, ha a Beállításokban az automatikus e-mailek be vannak kapcsolva. Kikapcsolt állapotban sorba állított levél nem várakozik, hanem törlődik.',
          'A levelek munkaidőben (alapból 8–17, hétköznap) mennek, címzettenkénti napi limittel. A tiltólistán lévőknek nem megy levél.',
          'Sablonok: területenként (értékesítés, projektek, számlázó…), mappákba rendezve, kukával. Változók (pl. ügyfél neve) szúrhatók be; az előnézet mutatja a feloldatlanokat.',
          'Ügyfélválasz: az értékesítési postafiók olvasása automatikus (kb. 10 percenként). A válasz a megfelelő lead/megrendelés/partner előzményeibe kerül, és leállítja az utánkövetőket. Ami semmire nem illik, azt nem tároljuk.',
        ],
      },
    ],
    links: [
      { label: 'E-mailek', href: '/hu/emails' },
      { label: 'Sablonok', href: '/hu/templates' },
    ],
  },
  {
    slug: 'hirlevel',
    group: 'Levelezés',
    title: 'Hírlevél: feliratkozók és kiküldések',
    summary: 'Dupla feliratkozás, címkék, időzített kiküldés, megnyitások és kattintások.',
    keywords: ['hírlevél', 'feliratkozó', 'kiküldés', 'címke', 'leiratkozás', 'megnyitás'],
    body: [
      {
        paragraphs: [
          'A honlapról érkező feliratkozás kétlépcsős: a cím előbb várakozik, majd a megerősítő linkre (7 napig él) kattintva lesz aktív. Irodai kézi felvétel és bemásolt import azonnal aktív (a leiratkozottakat az import kihagyja).',
          'A feliratkozók címkékbe (szekciókba) sorolhatók; a honlapos űrlap listákat is megnevezhet. Minden olvasó saját példányt kap, saját leiratkozólinkkel.',
        ],
      },
      {
        heading: 'Kiküldés és mérés',
        paragraphs: [
          'Kiküldés a Marketing → Kiküldések alatt: mindenkinek vagy választott címkéknek, azonnal vagy időzítve. A Kiküldések mutatja a megnyitásokat, kattintásokat és leiratkozásokat.',
          'Leiratkozás egy kattintás a levélben lévő linkkel; kézzel e-mail-cím alapján is leiratkoztatható. A nyelvi változatoknál az ismert nyelvű olvasó a saját nyelvűt kapja.',
        ],
      },
    ],
    links: [
      { label: 'Marketing', href: '/hu/marketing' },
      { label: 'Sablonok', href: '/hu/templates' },
    ],
  },
  {
    slug: 'hr',
    group: 'HR',
    title: 'HR: dolgozók, szabadság, toborzás',
    summary: 'Névsor, dokumentum-lejáratok, szabadság-naptár, álláshirdetés és jelentkezések.',
    keywords: ['hr', 'dolgozó', 'szabadság', 'távollét', 'toborzás', 'állás', 'dokumentum'],
    body: [
      {
        paragraphs: [
          'A HR-menü csak adminisztrátornak és HR-hozzáféréssel rendelkezőnek látszik (a Felhasználók oldalon adható). A névsorban név, fotó, céges és személyes elérhetőség, állapot és belépési adatok vannak.',
          'A személyes/pénzügyi/családi adatok a „…adatokra vár" állapotokat hajtják. Az okmányokhoz (orvosi, szerződés, jogosítvány, képzés) lejárat dátuma adható — a HR 30 nappal előtte és a napján értesítést kap.',
        ],
      },
      {
        heading: 'Szabadság és toborzás',
        paragraphs: [
          'Szabadság, betegszabadság, fizetetlen és egyéb távollét rögzíthető; a csapatnaptár a hónapot mutatja. Éves keret (alapból 20 nap) és egyenleg dolgozónként. Hétvége és magyar ünnepnap nem fogyaszt keretet; az áthidaló (ledolgozós szombati) napokat kézzel kell igazítani. Átfedő távollét nem rögzíthető.',
          'Toborzás: a hirdetéshez nyilvános, kitalálhatatlan link tartozik (/hu/jobs/…), általános jelentkezési űrlappal (név, elérhetőség, kor, város, üzenet, önéletrajz PDF/DOC/DOCX/ODT/RTF, max. 10 MB). Piszkozat hirdetésre nem lehet jelentkezni. Minden jelentkezésről értesítés megy a HR-nek.',
        ],
      },
    ],
    links: [{ label: 'HR', href: '/hu/hr' }],
  },
  {
    slug: 'kereses-ertesites',
    group: 'Mindennapi használat',
    title: 'Keresés, értesítések, jelentések',
    summary: 'Fejléc-keresés és parancspaletta, csengő, riportok, asszisztens.',
    keywords: ['keresés', 'értesítés', 'csengő', 'jelentés', 'riport', 'asszisztens', 'parancs'],
    body: [
      {
        paragraphs: [
          'Keresés: a fejléc mezője (/ billentyűvel fókuszálható) vagy a parancspaletta (Ctrl+K). Megrendelést, partnert, leadet, kapcsolattartót, e-mailt keres (HR-joggal dolgozót is). Minden szóra egyeznie kell, sorrend mindegy; ékezet és írásjelek nem számítanak (kovacs gyor megtalálja Kovács Győrt), telefonszám és rendszám bárhogy írva.',
          'Értesítések: a menüben a csengő mutatja az olvasatlanokat (30 másodpercenként frissül). Új honlapos lead, lejáró ajánlat, jelentkezés és lejáró dolgozói okmány generál értesítést. A lista 60 napig őrződik. A telefon kb. 15 percenként kérdez le (ez az Android alsó határa).',
        ],
      },
      {
        heading: 'Jelentések és asszisztens',
        paragraphs: [
          'Jelentések: műhelyterhelés, forgalom, fázisidők, elakadt munkák, akadályterhelés, lead-források, webes konverzió, értékesítési tölcsér, munkatársi számok, első válaszidő, országos forgalom (MNB-árfolyamon forintosítva), hírlevél-trendek. Minden grafikon saját időszakot állít.',
          'Asszisztens: ha be van kapcsolva, a lebegő panel adat-kérdésekre válaszol és listaszűrőket épít, amelyek egy kattintással megnyithatók vagy mentett nézetként elmenthetők. Csak olvas — írni nem tud, az adat nem hagyja el az épületet.',
        ],
      },
    ],
    links: [
      { label: 'Jelentések', href: '/hu/reports' },
      { label: 'Értesítések', href: '/hu/notifications' },
    ],
  },
  {
    slug: 'hasznalati-tippek',
    group: 'Mindennapi használat',
    title: 'Használati tippek: nézetek, export, gyorsbillentyűk',
    summary: 'Mentett nézetek, Excel/CSV-export, piszkozatok, gyorsbillentyűk, offline telefon.',
    keywords: ['nézet', 'export', 'excel', 'csv', 'piszkozat', 'billentyű', 'offline', 'sűrűség'],
    body: [
      {
        paragraphs: [
          'Mentett nézetek: a listákon beállított szűrők névvel elmenthetők és egy kattintással visszahívhatók. Oszlopok és táblázatsűrűség szintén személyre szabhatók.',
          'Export: minden listáról Excel (.xlsx), CSV, Google Táblázatokba másolás vagy vágólap. Nagy listánál az export az összes találatot végiglapozza (max. 20 ezer sor).',
          'Piszkozat: új rögzítéskor a félbehagyott űrlap automatikusan mentődik és visszatöltődik; beküldéskor törlődik. Elhagyáskor őr kérdez rá a mentetlen változásra.',
        ],
      },
      {
        heading: 'Billentyűk és telefon',
        paragraphs: [
          'Billentyűparancsok: / keresés, g o megrendelések, g l leadek, g d irányítópult, g e e-mailek, g b kijelző, g r jelentések, g k tudástár, n új rögzítés a listán, ? súgó.',
          'Telefon: ha nincs kapcsolat, az utolsó jó másolat látszik „milyen régi" jelzéssel. Szerkesztéshez kapcsolat kell; a fotófeltöltés és a vizsgálat sorba áll. Kijelentkezéskor a másolat törlődik.',
          'Naptár: a Saját beállításokban naptár-csatorna készíthető a saját teendőkről és határidőkről.',
        ],
      },
    ],
    links: [
      { label: 'Saját beállítások', href: '/hu/preferences' },
      { label: 'Tudástár', href: '/hu/tudastar' },
    ],
  },
  {
    slug: 'jogosultsagok',
    group: 'Mindennapi használat',
    title: 'Ki mit láthat? Szerepkörök röviden',
    summary: 'Admin, irodai, tervező, megtekintő — és a HR-kapcsoló.',
    keywords: ['jogosultság', 'szerepkör', 'admin', 'hr', 'hozzáférés'],
    body: [
      {
        paragraphs: [
          'Négy szerepkör van: Adminisztrátor (mindent lát és állít), Irodai (értékesítés, számlázás, levelezés), Tervező (gyártási adatok, fázisok, fotók, megjegyzések), Megtekintő (csak olvas).',
          'A HR külön kapcsoló a Felhasználók oldalon — HR-adatokat csak az lát, akinek ez be van kapcsolva (admin mindig). Egyedi pluszjogok is adhatók személyenként.',
          'Ha valamit nem látsz vagy egy gomb nem működik, szinte mindig jogosultság az oka — szólj az adminisztrátornak. Az utolsó aktív admin nem kapcsolható ki (a rendszer védi).',
        ],
      },
    ],
    links: [
      { label: 'Felhasználók', href: '/hu/admin' },
      { label: 'Saját beállítások', href: '/hu/preferences' },
    ],
  },
];
