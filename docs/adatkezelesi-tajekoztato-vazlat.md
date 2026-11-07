# Adatkezelési tájékoztató: VÁZLAT

> **Ez egy munkaanyag, nem kész jogi dokumentum.** A rendszer tényleges adatkezeléséből
> készült (mit tárol az AutoCRM, hol, kinek adja át), hogy a jogász vagy az adatvédelmi
> tanácsadó ne a nulláról kezdje. Közzététel előtt át kell nézetni vele. A `[szögletes
> zárójelben]` részeket a cégnek kell kitöltenie vagy eldöntenie. A rövid, közérthető
> változat a CRM nyilvános oldala: `/hu/adatkezeles`; ez a dokumentum az, amire az az oldal
> hivatkozik ("A teljes Adatkezelési tájékoztató").

## 1. Az adatkezelő

| | |
|---|---|
| Név | [Autotherm … Kft.] |
| Székhely | [cím] |
| Cégjegyzékszám / adószám | [szám] |
| E-mail | [adatvédelmi kapcsolattartó e-mail-címe] |
| Telefon | [szám] |
| Adatvédelmi tisztviselő | [Nincs kinevezve / név és elérhetőség. Döntendő, kötelező-e.] |

Az adatokat az Autotherm saját, általa üzemeltetett rendszerében (AutoCRM) kezeljük. Külső
CRM-szolgáltató nem kezel adatot az Autotherm helyett. (Az Autotherm korábban MiniCRM-et
használt; az onnan átvett adatokat a jelenlegi rendszer tartalmazza. [A MiniCRM-mel kötött
adatfeldolgozási szerződés megszűnésének és az adatok törlésének rendezése: ellenőrizendő.])

## 2. Az adatkezelések

Mindegyiknél: kinek az adata, milyen adat, mi a cél, mi a jogalap (GDPR 6. cikk), mennyi ideig
őrizzük. A megőrzési idők közül a *jogszabályi* a biztos, a többi **javaslat**, amit a cégnek
kell jóváhagynia.

| # | Érintettek | Adatok | Cél | Jogalap | Megőrzés |
|---|---|---|---|---|---|
| 1 | Ajánlatot kérők, érdeklődők (weboldal, telefon, e-mail) | Név, e-mail, telefonszám, üzenet, jármű adatai, az űrlap oldala, érkezési forrás (UTM-címkék, hivatkozó oldal, első oldal) | Az érdeklődés megválaszolása, ajánlatadás; a marketing eredményességének mérése | (b) szerződéskötést megelőző lépések; (f) jogos érdek (megválaszolás, marketing-mérés) | [Javaslat: az utolsó kapcsolatfelvételtől 2 év, ha nem lett belőle megrendelés] |
| 2 | Ügyfelek, partnerek, kapcsolattartóik | Név, cím, adószám, EU-adószám, e-mail, telefon, pozíció, megrendelések, ajánlatok, levelezés | Szerződés teljesítése, kapcsolattartás | (b) szerződés; (c) jogi kötelezettség | A szerződés megszűnése után [javaslat: 5 év] (polgári jogi elévülés); a számviteli bizonylatoké lásd a 4. pontot |
| 3 | Járművek tulajdonosai, üzembentartói | Rendszám, alvázszám, átvételi és átadási fotók, jegyzőkönyvek aláírással, a sérülések rögzítése | A vállalt munka és a jármű állapotának dokumentálása | (b) szerződés; (f) jogos érdek (vitás esetek, felelősség) | [Javaslat: a garanciális idő végéig, legalább 5 év] |
| 4 | Számlán szereplő személyek | Számlák adatai, a NAV-nak jelentett adatok | Számlázás, számviteli és adózási kötelezettségek | (c) jogi kötelezettség | **8 év** (számviteli törvény) |
| 5 | Hírlevélre feliratkozók | E-mail, név, a feliratkozás és megerősítés ideje | Hírlevél küldése | (a) hozzájárulás (kétlépcsős megerősítéssel); visszavonható | A leiratkozásig; a leiratkozás tényét [javaslat: további 5 évig] megőrizzük, hogy ne iratkoztassuk fel újra |
| 6 | Akivel levelezünk | E-mail-cím, a küldött levelek naplója (tárgy, tartalom, időpont, állapot) | Kapcsolattartás, annak igazolása, mit küldtünk | (b) szerződés; (f) jogos érdek | A kapcsolódó ügyével együtt |
| 7 | Munkatársak | **Külön tájékoztató**: név, kapcsolattartási adatok, fotó, szabadságok és távollétek, az HR-modulban tárolt iratok | Munkaviszony | [Munkáltatói tájékoztató, a munkaügyi jogszabályok szerint] | [Munkaügyi jogszabályok] |
| 8 | A rendszer felhasználói (munkatársak) | Név, e-mail, szerepkör, bejelentkezési munkamenetek (eszköz, IP-cím, időpont), a módosítások naplója | Hozzáférés-kezelés, biztonság, visszakövethetőség | (f) jogos érdek (a rendszer biztonsága) | [Javaslat: a munkaviszony végéig, a napló 2 évig] |

### Megjegyzések, amelyeket el kell dönteni

- **Érkezési forrás (1. sor).** A weboldal az érkezési adatokat a böngésző munkamenet-tárolójából
  (`sessionStorage`) olvassa ki. Az ePrivacy-szabályok (Eht. 155. §) a végberendezésen történő
  tárolásra és az onnan való olvasásra is vonatkoznak, ezért a **weboldal sütitájékoztatójában
  és a hozzájárulás kezelésében** is szerepeltetni kell. Ezt a weboldal fejlesztőjével és a
  jogásszal kell egyeztetni. A CRM csak azt tárolja, amit az űrlap elküld.
- **Fotók.** Az átvételi fotókon előfordulhatnak személyek vagy rendszámok. A rendszer az
  átvételi fotókat módosíthatatlanul őrzi (bizonyítékként), és a megjelenített másolatokból az
  EXIF-adatokat (köztük a helyadatot) eltávolítja. A fotók törölhetőségét az (f) jogalap
  és a megőrzési idő határozza meg; [a törlési kérelmek kezelésének rendje: döntendő].
- **Az átvételi fotók nem törölhetők.** Az adatbázis szándékosan megtagadja az átvételi
  fotók módosítását és törlését (bizonyítékként őrizzük őket), ezért egy törlési kérelem erre
  a rendszerben nem hajtható végre. Ezt a tájékoztatóban meg kell indokolni (jogos érdek /
  jogi igények érvényesítése, GDPR 17. cikk (3) e)), és meg kell határozni, mikor és hogyan
  szüntethető meg a megőrzés (a megőrzési idő letelte után a törlés közvetlen adatbázis-
  művelettel, a vezetőség jóváhagyásával lehetséges).
- **Hírlevél.** Közvetlen üzletszerzési célú megkereséshez a hozzájárulás kell (Grt. 6. §).
  A rendszer kétlépcsős megerősítést (double opt-in) használ, és minden levélben ott a
  leiratkozás. A hírlevél űrlapja mellett fel kell tüntetni az adatkezelési tájékoztatóra
  mutató hivatkozást, és a hozzájáruló jelölőnégyzet **nem lehet előre kipipálva**.

## 3. Címzettek és adatfeldolgozók

Az adatokat nem adjuk el és nem használjuk fel más célra. A következők kaphatnak adatot:

| Címzett | Mire | Mit | Megjegyzés |
|---|---|---|---|
| NAV (Online Számla) | Számlaadat-szolgáltatás | Számlák adatai | Jogszabályi kötelezettség; önálló adatkezelő |
| E-mail szolgáltató: [Google Workspace] | Levelek küldése és fogadása | Az e-mail-címek és a levelek tartalma | Adatfeldolgozó. [A Google adatfeldolgozási kiegészítésének elfogadása és az adattovábbítási mechanizmus (EU–USA adatvédelmi keretrendszer, SCC) ellenőrizendő.] |
| Tárhely- és üzemeltetési szolgáltató: [név, ország] | A rendszer és az adatbázis futtatása | Minden, a rendszerben tárolt adat | Adatfeldolgozó. [Adatfeldolgozási szerződés kell; az adatok helye: …] |
| Objektumtárolás (fotók, dokumentumok): [szolgáltató, ország] | A fotók és fájlok tárolása | Fotók, aláírások, dokumentumok | Adatfeldolgozó. [Ugyanez] |
| Könyvelő: [név] | Könyvelés | Számlázási adatok | [Adatfeldolgozó vagy önálló adatkezelő: döntendő] |
| [Más, pl. hirdetési és elemző szolgáltatók a weboldalon] | | | A weboldal tájékoztatójába tartozik |

**Harmadik országba** történő továbbítás: [csak akkor, ha a fenti szolgáltatók közül valamelyik
az EGT-n kívül kezel adatot; ott a megfelelő garanciát (határozat, SCC) meg kell nevezni].

## 4. Az adatok biztonsága

Amit a rendszer ténylegesen tesz (ellenőrizhető a forráskódban, ezek nem ígéretek):

- A belépés egyéni fiókkal történik; a jelszavakat erős, sózott egyirányú függvénnyel (Argon2)
  tároljuk, és az ideiglenes jelszót az első belépéskor le kell cserélni.
- A webes munkamenet `httpOnly` és (éles környezetben) `Secure` sütiben él, lejár, és az
  adminisztrátor visszavonhatja. Sikertelen belépések után a fiók átmenetileg zárolódik.
- A jogosultságok szerepkörökhöz kötöttek (adminisztrátor, iroda, tervező, megtekintő). A
  munkatársak személyes adatait tartalmazó HR-modul külön hozzáféréshez kötött, amit csak az
  adminisztrátor adhat meg.
- A módosítások naplózva vannak (ki, mikor, mit). A HR-adatok naplója a módosított mezők
  nevét tartalmazza, az értéküket nem.
- A fájlok feltöltése aláírt, lejáró jegyekkel történik; a megjelenítéshez rövid életű
  hivatkozásokat használunk.
- Az átvételi fotók módosíthatatlanok, a megjelenített másolatok nem tartalmaznak helyadatot.
- [Az éles környezet technikai védelme (titkosított kapcsolat, mentések, hozzáférés a
  szerverhez, mentések titkosítása): az üzemeltetési leírásból kell kiegészíteni.]

## 5. Az érintettek jogai

Hozzáférés, helyesbítés, törlés, az adatkezelés korlátozása, tiltakozás (különösen a jogos
érdeken alapuló kezelés és a közvetlen üzletszerzés ellen), adathordozhatóság, a hozzájárulás
visszavonása. A kérésre **legfeljebb egy hónapon belül** válaszolunk (GDPR 12. cikk), amely
bonyolult esetben meghosszabbítható. [A kérelmek fogadásának és nyilvántartásának belső rendje:
ki kapja, ki dönt, hol rögzítjük.]

A hírlevélről a levelekben lévő hivatkozással vagy a `/hu/newsletter/unsubscribe` oldalon
iratkozhatsz le.

## 6. Jogorvoslat

Panasz: Nemzeti Adatvédelmi és Információszabadság Hatóság (NAIH), www.naih.hu. [A hatóság
aktuális címét és elérhetőségét a közzététel előtt ellenőrizni kell.] Bírósághoz is lehet
fordulni; a per az érintett választása szerint a lakóhelye vagy tartózkodási helye szerinti
törvényszéken is indítható.

## 7. A rendszerből adódó teendők (technikai lista)

Ami a tájékoztatóból **a rendszerben** is megvalósítandó vagy ellenőrizendő:

- [ ] A weboldal űrlapja mellett legyen hivatkozás: `https://[CRM címe]/hu/adatkezeles`.
- [ ] A hírlevél-feliratkozás jelölőnégyzete nincs előre kipipálva.
- [ ] Az e-mail-sablonok láblécében legyen az Adatkezelés hivatkozás (Beállítások → sablonok).
- [ ] A törlési és hozzáférési kérelmek kezelésére legyen belső eljárás (a CRM-ben ma nincs
      "az érintett összes adatának kigyűjtése/törlése" funkció; kézzel kell elvégezni).
- [ ] Az adatok megőrzési idejének letelte után a régi érdeklődők és a lejárt adatok törlése
      (ma nincs automatikus törlés).
- [ ] A PRIVACY_* környezeti változók kitöltése (`frontend/.env.example`), hogy a nyilvános
      oldalon megjelenjenek a cég adatai.
