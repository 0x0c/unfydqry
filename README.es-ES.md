# unfydqry

> 🌐 Versión en español: [docs/es/README.md](docs/es/README.md) (En preparación)

Un motor de búsqueda de texto completo compartido utilizable tanto en iOS (SwiftData) como en Android (Room).
Un único núcleo de búsqueda escrito en **Rust + UniFFI** que se consume como un paquete SwiftPM en iOS y como un módulo Gradle en Android.

La justificación del diseño se encuentra en [`docs/cross-platform-search-engine-design.md`](docs/cross-platform-search-engine-design.md).

[![Swift Tests](https://github.com/0x0c/unfydqry/actions/workflows/swift-tests.yml/badge.svg)](https://github.com/0x0c/unfydqry/actions/workflows/swift-tests.yml)
[![Kotlin Tests](https://github.com/0x0c/unfydqry/actions/workflows/kotlin-tests.yml/badge.svg)](https://github.com/0x0c/unfydqry/actions/workflows/kotlin-tests.yml)
[![Rust Tests](https://github.com/0x0c/unfydqry/actions/workflows/rust-tests.yml/badge.svg)](https://github.com/0x0c/unfydqry/actions/workflows/rust-tests.yml)
[![Flutter Tests](https://github.com/0x0c/unfydqry/actions/workflows/flutter-tests.yml/badge.svg)](https://github.com/0x0c/unfydqry/actions/workflows/flutter-tests.yml)

## Qué hace

- **Comportamiento enchufable**: el binding del host elige un *perfil de normalización* y un *algoritmo de búsqueda*, y el motor los combina. Ambas implementaciones residen en un único núcleo de Rust, por lo que cualquier combinación elegida se comporta idénticamente en iOS y Android; consulte [Configuración del comportamiento](#configuracion-del-comportamiento).
- **Ejes de imprecisión (fuzziness) que se pliegan** (perfil `loose` por defecto): mayúsculas/minúsculas, ancho completo/medio, variante de kana (katakana ↔ hiragana).
- **Los dakuten / handakuten se mantienen distintos** (`か` y `が` son claves diferentes).
- **La búsqueda predeterminada** es un índice SQLite FTS5 + trigrama clasificado por `bm25`; también se pueden seleccionar algoritmos de subcadena, prefijo, sufijo, todos los términos y difusos (trigrama / Levenshtein / Damerau-Levenshtein).
- Las búsquedas devuelven solo el `id` estable y una puntuación; el host vuelve a recuperar los registros desde su almacén de verdad (source-of-truth).
- **Registros multi-campo**: indexa los campos de un registro por separado y consulta en todos ellos en una sola llamada, aprendiendo *qué* campo coincidió; consulte [Registros multi-campo (API de capa de registros)](#registros-multi-campo-api-de-capa-de-registros).
- Debido a que la lógica reside en **una sola implementación de Rust**, el comportamiento de iOS y Android coincide por construcción, no por convención.

## Arquitectura

La idea central —y la razón principal por la que existe esta librería— es que **toda la lógica de búsqueda reside en un único núcleo de Rust**, consumido a través de bindings de UniFFI generados automáticamente. Swift y Kotlin no pueden derivar hacia implementaciones diferentes, por lo que la consistencia multiplataforma es una propiedad *estructural* en lugar de algo mantenido por disciplina.

```
┌─────────────────────────────┐     ┌─────────────────────────────┐
│  App de iOS                  │     │  App de Android              │
│  ┌────────────────────────┐ │     │  ┌────────────────────────┐ │
│  │ Almacén primario (verdad)│ │     │  │ Almacén primario (verdad)│ │
│  └───────────┬────────────┘ │     │  └───────────┬────────────┘ │
│              │ indexar/eliminar  │     │              │ indexar/eliminar │
│  ┌───────────▼────────────┐ │     │  ┌───────────▼────────────┐ │
│  │ SearchEngine (Swift)   │ │     │  │ SearchEngine (Kotlin)   │ │
│  └───────────┬────────────┘ │     │  └───────────┬────────────┘ │
└──────────────┼──────────────┘     └──────────────┼──────────────┘
               │                                    │
        ┌──────▼────────────────────────────────────▼──────┐
        │      Núcleo de Rust (UniFFI) — una sola impl física  │
        │  normalización / gestión de índice / ranking / matching  │
        └───────────────────────────────────────────────────┘
        Índice de búsqueda (un archivo separado del almacén primario)
```

De esto se derivan dos elecciones estructurales:

- **Dueño del índice, agnóstico al almacén.** El motor posee su propio índice de búsqueda, mantenido separado de su almacén de verdad. SwiftData / Room son solo ejemplos; los datos primarios pueden residir en cualquier lugar; el motor solo requiere que cada registro pueda recuperarse mediante un `id` estable. Los resultados de búsqueda devuelven ese `id` más una puntuación, y el host recupera el registro completo.
- **Runtime empaquetado y sin diccionarios.** La normalización y el sustrato de búsqueda (SQLite/FTS5) se compilan en el núcleo en lugar de tomarse del SO, por lo que los resultados no varían según las versiones del SO o del dispositivo. Una [`spec/`](spec/README.md) compartida es verificada por el CI de cada plataforma, por lo que cualquier deriva en el núcleo falla el *mismo caso* en todas partes simultáneamente.

Justificación completa: [`docs/cross-platform-search-engine-design.md`](docs/cross-platform-search-engine-design.md).

## Estructura

```
unfydqry/
├── Package.swift                ← Punto de entrada SwiftPM, mantenido en la raíz del repo
├── core/                        Implementación en Rust (nombre del crate: unfydqry)
│   ├── Cargo.toml
│   ├── src/lib.rs               Superficie FFI (constructores, exportaciones normalize*)
│   ├── src/config.rs           NormalizeProfile / NormalizeOptions / SearchStrategy / EngineConfig / EngineOptionsConfig
│   ├── src/engine.rs           SearchEngine (index/search/remove/reindex + index_record/search_records/remove_record/change_field_bits de la capa de registros, retención de texto plano, sellos de normalize + field_bits)
│   ├── src/normalize/          pasos de normalización componibles (steps.rs) + presets
│   ├── src/search/             algoritmos de consulta intercambiables (trigram_bm25/substring/prefix/suffix/all_terms/fuzzy_trigram/levenshtein/damerau_levenshtein)
│   ├── src/bin/uniffi-bindgen.rs
│   └── tests/conformance.rs     pruebas de integración basadas en spec (ver Pruebas)
├── spec/                        especificación de pruebas multiplataforma (JSON)
│   ├── README.md                esquema y convenciones
│   ├── normalize.json           (entrada → esperado) para normalizeLoose
│   └── search.json              escenarios + matrices sembradas para SearchEngine
├── ios/                         todo lo específico de iOS
│   ├── UnifiedQuery.xcframework  artefacto de build (.gitignore)
│   ├── Sources/UnifiedQuery/     librería SwiftPM; el binding está commitado
│   ├── Tests/UnifiedQueryTests/  Swift Testing — 4 suites (ver Pruebas)
│   └── sample/                   app de muestra SwiftUI (consume el paquete)
├── android/
│   ├── jniLibs/                 libunfydqry.so producido por cargo-ndk (.gitignore)
│   └── sample/                  raíz de Gradle
│       ├── settings.gradle.kts  include(":app", ":unifiedquery")
│       ├── app/                 app de muestra Compose
│       └── unifiedquery/        librería JVM Kotlin + JUnit 5 — 4 clases
├── flutter/                     plugin de Flutter (paquete Dart: unfydqry)
│   ├── lib/unfydqry.dart        API pública de Dart (SearchEngine, Hit, RecordHit, FieldValue, SearchException)
│   ├── ios/                     plugin Swift → UnifiedQuery.SearchEngine
│   ├── android/                 plugin Kotlin → uniffi.unfydqry.SearchEngine
│   ├── test/                    pruebas unitarias de Dart con mock-channel
│   └── example/                 app de muestra Flutter (misma semilla de 8 registros)
└── docs/
    ├── ios.md                    guía de iOS (Swift) — instalación / uso / build / pruebas / release
    ├── android.md                guía de Android (Kotlin) — instalación / uso / build / pruebas / release
    ├── flutter-plugin.md
    ├── cross-platform-search-engine-design.md   justificación del diseño (inglés)
    └── ja/                       documentación en japonés
        ├── README.md             README en japonés
        └── cross-platform-search-engine-design.md   justificación del diseño (japonés)
```

| | iOS | Android |
|---|---|---|
| Librería | `import UnifiedQuery` (SwiftPM) | `implementation(project(":unifiedquery"))` |
| Binding generado | `ios/Sources/UnifiedQuery/UnifiedQuery.swift` | `android/sample/unifiedquery/src/main/kotlin/uniffi/unfydqry/unfydqry.kt` |
| Módulo FFI | `unfydqryFFI` (vía el modulemap dentro del XCFramework) | `libunfydqry.so` cargado a través de JNA |
| Distribuible | `ios/UnifiedQuery.xcframework` (arm64 device + arm64/x86_64 sim + arm64 mac) | `android/jniLibs/{arm64-v8a,armeabi-v7a,x86_64}/libunfydqry.so` |

## Guías de plataforma

La configuración por plataforma, fragmentos de uso rápido, builds de artefactos nativos, diseño de pruebas y flujo de release residen cada uno en una guía dedicada. Las secciones multiplataforma a continuación (configuración de comportamiento, el contrato de pruebas `spec/`) se aplican a todos los bindings.

| Plataforma | Guía | Librería |
|---|---|---|
| iOS (Swift) | [`docs/ios.md`](docs/ios.md) | `import UnifiedQuery` (SwiftPM) |
| Android (Kotlin) | [`docs/android.md`](docs/android.md) | `io.github.0x0c:unifiedquery` (Gradle / Maven Central) |
| Flutter (Dart) | [`docs/flutter-plugin.md`](docs/flutter-plugin.md) | `unfydqry` (paquete Dart, dependencia de Git) |

## Configuración del comportamiento

`SearchEngine` tiene cinco constructores. La **combinación se elige en el lado del binding**; cada implementación reside en el núcleo de Rust (`core/src/normalize/`, `core/src/search/`), por lo que la elección nunca puede hacer que iOS y Android diverjan.

- `SearchEngine(dbPath:)` — la combinación predeterminada, `loose` + `trigram_bm25`. Sin cambios respecto a versiones anteriores, por lo que los llamadores existentes siguen funcionando.
- `SearchEngine.withConfig(dbPath:, config:)` — elige un **perfil** de normalización y el algoritmo de búsqueda. Abrir un índice bajo un perfil *diferente* es un error (ver más abajo).
- `SearchEngine.withConfigRebuilding(dbPath:, config:)` — igual que `withConfig`, pero un cambio de normalización regenera el índice en el lugar en vez de dar error (ver [Regeneración del índice](#regeneracion-del-indice-despues-de-un-cambio-de-normalizacion)).
- `SearchEngine.withOptions(dbPath:, config:)` — como `withConfig`, pero selecciona la normalización con un conjunto componible de `NormalizeOptions` (ver más abajo) en lugar de un preset con nombre.
- `SearchEngine.withOptionsRebuilding(dbPath:, config:)` — `withOptions` + regeneración en el lugar ante un cambio de normalización.

### Perfiles de normalización (`NormalizeProfile`)

El perfil se aplica idénticamente en el momento de indexar y en el momento de consultar.

| Perfil | Pipeline | Efecto |
|---|---|---|
| `loose` (predeterminado) | NFKC → katakana→hiragana → minúsculas | Mayúsculas, ancho y variante de kana se pliegan juntos; `ﾄｳｷｮｳ`, `トウキョウ`, `とうきょう` colapsan en una sola clave. |
| `nfkc_case_fold` | NFKC → minúsculas | El ancho y las mayúsculas se pliegan, pero **las variantes de kana permanecen distintas** (`トウキョウ` ≠ `とうきょう`). |

Ambos perfiles mantienen los dakuten / handakuten distintos (`か` ≠ `が`).

### Pasos de normalización componibles (`NormalizeOptions`)

Para un control más fino, `withOptions` toma un conjunto `NormalizeOptions`: NFKC siempre se aplica como base, y cualquiera de los siguientes pasos puede activarse encima. Los dos perfiles anteriores son solo presets con nombre: `loose` = `{lowercase, kana_fold}`, `nfkc_case_fold` = `{lowercase}`.

| Paso | Efecto |
|---|---|---|
| `lowercase` | Pliegue de mayúsculas vía `char::to_lowercase`. |
| `kana_fold` | Katakana → hiragana (`カ` → `か`); los dakuten permanecen distintos. |
| `fold_diacritics` | Elimina marcas de combinación latinas/occidentales (`café` → `cafe`); se preservan las marcas vocálicas japonesas. |
| `fold_choonpu` | Pliega la marca de sonido prolongado después de los kana (`サーバー` ≡ `サーバ`). |
| `expand_iteration_marks` | Expande las marcas de iteración (`時々` → `時時`, `こゞ` → `こご`). |
| `normalize_hyphens` | Unifica la familia de guiones/rayas (`‐ – — −` …) al ASCII `-`. |
| `strip_digit_grouping` | Elimina las comas de agrupación de dígitos (`1,000` → `1000`). |
| `collapse_whitespace` | Colapsa secuencias de espacios en blanco en un solo espacio y recorta los extremos. |

Los pasos habilitados se ejecutan en un orden canónico fijo (`NFKC → expand_iteration_marks → kana_fold → fold_choonpu → lowercase → fold_diacritics → normalize_hyphens → strip_digit_grouping → collapse_whitespace`), por lo que cualquier combinación es determinista e idéntica en iOS y Android.

> La normalización activa se registra como una huella digital (fingerprint) en la tabla `meta` del índice. Los dos presets mantienen sus claves históricas (`loose` / `nfkc_case_fold`); cualquier otra combinación deriva una clave canónica `nfkc+…`. Abrir un índice existente bajo una huella *diferente* lanza `ConfigMismatch` en lugar de devolver resultados incorrectos silenciosamente; regenere el índice para cambiar (ver más abajo). (Un índice creado antes de que existiera este campo se trata como `loose`).

### Regeneración del índice después de un cambio de normalización

El motor almacena el **texto plano** de cada documento junto con su forma normalizada, por lo que el índice puede regenerarse en el lugar cuando el perfil (o sus reglas subyacentes) cambia; el host no tiene que volver a alimentar los documentos.

- **Explícita** — llame a `reindex()` en un motor abierto. Normaliza nuevamente cada documento almacenado bajo el perfil actual del motor, sobrescribe el índice y vuelve a sellar la huella del perfil. Devuelve el número de documentos regenerados.
- **Automática al abrir** — `SearchEngine.withConfigRebuilding` / `withOptionsRebuilding` abren el índice y, cuando la huella almacenada difiere de la solicitada, ejecutan la misma regeneración antes de retornar en lugar de lanzar `ConfigMismatch`.

> Los documentos indexados antes de que existiera la retención de texto plano no tienen texto plano para re-normalizar y no se ven afectados por una regeneración.

### Algoritmos de búsqueda (`SearchStrategy`)

Cada algoritmo se ejecuta contra el texto ya normalizado y devuelve `(id, score)`.

| Estrategia | Coincide | Cómo | Puntuación | Ideal para |
|---|---|---|---|---|
| `trigram_bm25` (predeterminado) | toda la consulta como frase, en cualquier lugar del texto | índice de trigramas FTS5 + `bm25()` | relevancia bm25 (menor = más relevante) | Búsqueda de texto completo **clasificada** de propósito general. |
| `substring` | la consulta en cualquier lugar del texto | `LIKE '%q%'` | `0.0` (no clasificado) | Coincidencias de "contiene" donde las consultas cortas (1–2 caracteres) también deben coincidir y el ranking no importa. |
| `prefix` | texto que **comienza con** la consulta | escaneo de rango de índice B-tree | `0.0` (no clasificado) | Sugerencias de autocompletado / type-ahead. |
| `suffix` | texto que **termina con** la consulta | `LIKE '%q'` | `0.0` (no clasificado) | Coincidencias de "termina con" (ej. extensiones de archivo, sufijos honoríficos). |
| `all_terms` | docs que contienen **cada** término separado por espacios, en cualquier orden | `LIKE '%t%'` con AND por término | `0.0` (no clasificado) | Consultas de varias palabras donde el orden de las palabras es irrelevante (a diferencia de `substring`, que requiere la secuencia literal incluyendo espacios). |
| `fuzzy_trigram` | docs cuyo conjunto de trigramas de caracteres es lo suficientemente similar a la consulta (Jaccard ≥ umbral) | pre-filtro FTS5 + Jaccard en Rust | `1 − similitud` (menor = más similar; exacto = `0.0`) | Tolerancia a errores tipográficos sin un escaneo completo de distancia de edición. |
| `levenshtein` | docs con una palabra dentro de un umbral de distancia de edición de la consulta | distancia Levenshtein mínima a cualquier palabra, en Rust | distancia de edición (menor = mejor) | Coincidencia tolerante a errores tipográficos de una sola palabra/término. |
| `damerau_levenshtein` | igual que `levenshtein`, pero una transposición adyacente cuenta como una edición | distancia OSA mínima a cualquier palabra, en Rust | distancia de edición (menor = mejor) | Tolerancia a errores que también perdona caracteres vecinos intercambiados (`tokoy` ↔ `tokyo`). |

Notas:
- Las estrategias **clasificadas (ranked)** son `trigram_bm25` (por bm25), `fuzzy_trigram` (por similitud) y `levenshtein` / `damerau_levenshtein` (por distancia). `substring`, `prefix`, `suffix` y `all_terms` no están clasificadas (constante `0.0`, orden de almacenamiento); use `limit` para limitar los resultados.
- `trigram_bm25` no puede coincidir con consultas de menos de 3 caracteres, por lo que estas pasan automáticamente a un `LIKE` de subcadena (puntuación `0.0`).
- Las estrategias difusas no necesitan crates adicionales ni extensiones de SQLite. `fuzzy_trigram` usa el índice de trigramas FTS5 existente para reducir los candidatos antes de calcular la similitud de Jaccard en Rust; las distancias de edición se calculan en Rust sobre el texto normalizado (por punto de código Unicode, por lo que el japonés se compara correctamente) con terminación temprana cuando la distancia excede el umbral. El umbral de distancia de edición escala con la longitud de la consulta (1 edición por cada 4 caracteres, mínimo 1).

### Selección de una combinación

La combinación se elige en el lado del binding; consulte las llamadas por lenguaje en las guías de [iOS](docs/ios.md#selecting-a-combination), [Android](docs/android.md#selecting-a-combination) y [Flutter](docs/flutter-plugin.md).

Para inspeccionar la normalización directamente, también hay funciones libres: `normalizeLoose(input)` (siempre el perfil `loose`), `normalizeWithProfile(input, profile)` y `normalizeWithOptions(input, options)` para un conjunto de pasos componibles.

### Resaltado de regiones coincidentes

`highlight(query, id, before, after)` devuelve el texto original del host del documento con las regiones coincidentes envueltas en marcadores especificados por el llamador:

```swift
// iOS
let snippet = try engine.highlight(query: "検索", id: 1, before: "<b>", after: "</b>")
// → Optional("情報<b>検索</b>プログラム")
```

```kotlin
// Android
val snippet = engine.highlight("検索", 1L, "<b>", "</b>")
// → "情報<b>検索</b>プログラム"
```

Devuelve `nil` / `null` si el documento no existe o si la consulta normalizada está vacía. Cuando el documento existe pero la consulta no coincide, se devuelve el texto original sin marcadores.

La coincidencia se realiza sobre la forma normalizada (el mismo pliegue aplicado en el indexado y la búsqueda), pero las regiones marcadas se mapean de vuelta al texto plano del host, por lo que el resultado preserva las mayúsculas, el ancho y los kana originales en lugar de la forma plegada. Cuando una coincidencia cae dentro de un solo carácter de origen que se expandió bajo la normalización, el marcador se ajusta hacia afuera para cubrir ese carácter completo.

> **Nota:** Los documentos indexados antes de que se retuviera el texto plano no tienen texto plano al cual mapear; para esos, se marca directamente el texto normalizado.

### Recuento de coincidencias

`matchCount(query)` devuelve el número total de documentos que coinciden con la consulta, sin límite; útil para patrones de UI de "Aproximadamente N resultados".

```swift
// iOS
let total = try engine.matchCount(query: "とうきょう")
// → 42
```

```kotlin
// Android
val total = engine.matchCount("とうきょう")
// → 42
```

Devuelve `0` para consultas vacías o que solo contienen espacios en blanco. Para las estrategias basadas en SQL, el recuento se calcula con un `SELECT COUNT(*)` eficiente; para las estrategias difusas y de distancia de edición del lado de Rust, se ejecuta el paso de coincidencia completo internamente.

### Paginación

`searchPage(query, perPage, page)` devuelve una sola página de resultados (indexada en 0). Combine con `matchCount` para construir UIs paginadas.

```swift
// iOS
let total = try engine.matchCount(query: "とうきょう")
let page0 = try engine.searchPage(query: "とうきょう", perPage: 20, page: 0)
let page1 = try engine.searchPage(query: "とうきょう", perPage: 20, page: 1)
```

```kotlin
// Android
val total = engine.matchCount("とうきょう")
val page0 = engine.searchPage("とうきょう", 20u, 0u)
val page1 = engine.searchPage("とうきょう", 20u, 1u)
```

La página 0 devuelve los mismos resultados que `search(query, perPage)`. Las páginas más allá del conjunto de resultados devuelven una lista vacía.

### Gestión del índice

| Llamada | Qué hace |
|---|---|
| `documentCount()` | Devuelve el número total de documentos en el índice. Con la API de capa de registros, cada campo cuenta como un documento separado. |
| `removeAll()` | Elimina todos los documentos del índice y devuelve el número eliminado. Útil para reinicios de datos. |
| `contains(id)` | Devuelve si un documento con el `id` dado existe en el índice. |
| `indexBatch([IndexItem(id, text), …])` | Indexa muchos documentos `(id, text)` en una sola transacción; mucho más rápido que llamar a `index` por elemento en lotes grandes. Devuelve el número procesados. |
| `removeBatch([id, …])` | Elimina muchos ids en una sola transacción; los ids faltantes se omiten. Devuelve el número procesados. |

## Registros multi-campo (API de capa de registros)

`index` / `search` tratan cada `id` como un único bloque de texto. Cuando un registro tiene varios campos buscables —el nombre, la lectura y la nota de un contacto, por ejemplo— la **API de capa de registros** indexa cada campo por separado mientras sigue devolviendo un resultado por registro, por lo que una consulta puede coincidir con *cualquier* campo y usted puede saber *qué* campo coincidió.

Es una capa delgada sobre el mismo índice: el motor empaqueta `(record_id, slot)` en el id estable que almacena, y colapsa los hits de campo de vuelta a registros en el momento de la búsqueda. El id empaquetado nunca sale del motor; los hosts solo pasan un `record_id` (su propio `i64`) y un `slot` por campo (un `u8` pequeño y estable), y reciben `RecordHit { record_id, score, matched_slots }`.

| Llamada | Qué hace |
|---|---|
| `indexRecord(recordId, [FieldValue(slot, text), …])` | Upsert de un registro completo. Los campos que quedan vacíos tras la normalización se descartan; re-indexar el mismo `recordId` lo reemplaza completamente. Se rechazan los slots duplicados en una sola llamada. |
| `indexRecordsBatch([RecordIndexItem(recordId, [FieldValue(slot, text), …]), …])` | Upsert de muchos registros en una sola transacción. Todo o nada: si cualquier `recordId` o `slot` es inválido, no se indexa nada. Devuelve el número procesados. |
| `searchRecords(query, limit, fieldsPerRecord)` | Busca a través de los campos; devuelve como máximo `limit` `RecordHit`s clasificados por el mejor campo coincidente (menor puntuación) de cada registro. `fieldsPerRecord` es el recuento de campos del host, usado solo como pista de sobre-recuperación. |
| `removeRecord(recordId)` | Elimina cada campo de un registro. |
| `highlightRecord(query, recordId, slot, before, after)` | Resalta un campo específico de un registro. Devuelve `nil`/`null` si el slot no existe. |
| `matchCountRecords(query, fieldsPerRecord)` | Número total de *registros* que coinciden con la consulta (los hits de campos se colapsan en ids de registros únicos). |
| `searchRecordsPage(query, perPage, page, fieldsPerRecord)` | Búsqueda a nivel de registro paginada (indexada en 0). La página 0 es igual a `searchRecords(query, perPage, …)`. |
| `changeFieldBits(newFieldBits)` | Re-empaqueta todo el índice a un nuevo `field_bits` (ver más abajo). |

`index` / `remove` / `search` / `Hit` no han cambiado y aún pueden usarse directamente; la capa de registros es puramente aditiva.

### `field_bits`

El id empaquetado se divide en un `record_id` (bits altos) y un `slot` (los bits bajos `field_bits`). `field_bits` es **8** por defecto — hasta 256 campos por registro, dejando ~3.6×10¹⁶ ids de registro— y se elige por índice vía `EngineConfig.field_bits` / `EngineOptionsConfig.field_bits` (`Option<u8>`, rango válido `1..=62`):

- **Omitirlo** (`None`, el predeterminado): adopta cualquier valor con el que se haya creado el índice (o `8` para un índice nuevo). Esto nunca da error por field-bits, por lo que abrir un índice sin importar su empaquetado sigue funcionando, incluidos los llamadores simples de `index` / `search`.
- **Establecerlo** (`Some(n)`): requiere `n`; abrir un índice sellado con un valor *diferente* lanza `FieldBitsMismatch`.

`field_bits` se sella en el índice (como la huella de normalización) y se fija en la creación, porque determina la codificación del id. Para cambiarlo, llame a `changeFieldBits(n)`: re-empaqueta cada id almacenado en el lugar, todo o nada; si cualquier slot o id de registro almacenado no cupiera bajo `n`, nada cambia y devuelve un error.

> Elección de `field_bits`: elija el recuento más pequeño que albergue cómodamente sus campos. Los límites reales no son el espacio de ids (astronómicamente grande) sino el almacenamiento/latencia y la forma del `record_id`; los ids aleatorios o derivados de UUID rara vez encajan en el rango no negativo `0..=2^(63−field_bits)−1`, por lo que se prefieren ids secuenciales.

Las llamadas por lenguaje están en las guías de [iOS](docs/ios.md#record-layer-search-multi-field), [Android](docs/android.md#record-layer-search-multi-field) y [Flutter](docs/flutter-plugin.md#dart-api).

## Construcción (Build)

### Requisitos previos
- Rust estable (vía rustup)
- macOS + Xcode 26+ (para el lado de iOS)
- Android NDK r29+ y el Android SDK (para el lado de Android)
- JDK 17+ (para Gradle)

### Solo núcleo de Rust
```sh
cd core
cargo test --all-targets         # unitarias + conformidad
cargo build --release
```

### Benchmarks

El núcleo de Rust incluye benchmarks de [Criterion](https://github.com/bheisler/criterion.rs) que cubren búsqueda, indexación y normalización. Todos los benchmarks usan una base de datos SQLite en memoria con texto japonés generado determinísticamente.

```sh
cd core

# Ejecutar todos los benchmarks
cargo bench

# Ejecutar una suite de benchmarks específica
cargo bench --bench search       # estrategias de búsqueda (8 estrategias × 3 tamaños de corpus × 3 longitudes de consulta)
cargo bench --bench index        # indexación masiva, adjunción simple y re-indexación
cargo bench --bench normalize    # perfiles de normalización y pasos individuales

# Filtrar por un grupo o caso específico
cargo bench -- "search/trigram_bm25"
cargo bench -- "index/bulk"
cargo bench -- "normalize/profile"
```

Después de la primera ejecución, Criterion guarda los resultados base en `core/target/criterion/`. Las ejecuciones posteriores se comparan con la base y reportan regresiones. Los reportes HTML se generan en `core/target/criterion/report/index.html`.

### Construcciones de plataforma

La construcción de los artefactos nativos (XCFramework / `.so`) y las apps de muestra se cubre por plataforma:

- iOS (XCFramework + muestra Xcode) — [`docs/ios.md#build-swiftpm--xcode-sample`](docs/ios.md#build-swiftpm--xcode-sample)
- Android (`.so` vía cargo-ndk + muestra Gradle) — [`docs/android.md#build-gradle-sample`](docs/android.md#build-gradle-sample)
- Flutter — [`docs/flutter-plugin.md#building-native-artifacts`](docs/flutter-plugin.md#building-native-artifacts)

### Apps de muestra

Ambas apps de muestra (`ios/sample`, `android/sample/app`) demuestran la misma UX para que las dos plataformas puedan compararse lado a lado:

- Un campo de búsqueda estándar con **búsqueda incremental** (debounced ~150 ms); una consulta vacía enumera cada registro sembrado.
- **Registros multi-campo** sembrados en una sola transacción con `indexRecordsBatch` (un nombre + lectura por registro) y consultados con `searchRecords`; cada fila de resultado muestra qué campo coincidió, demostrando la API de capa de registros.
- **Operaciones por lote**: botones de adición masiva / eliminación masiva que añaden y eliminan un segundo conjunto de registros en una sola transacción (`indexRecordsBatch` / `removeBatch`).
- Un **modal de configuración** (SwiftUI `.sheet` / Compose `ModalBottomSheet`) con un interruptor por cada paso de `NormalizeOptions`, el selector de algoritmo de búsqueda y un botón de **regeneración de índice**. Al cambiar un paso, el índice se regenera en el lugar vía `withOptionsRebuilding`, por lo que los resultados se actualizan sin volver a alimentar los registros.
- La misma semilla (8 registros) en ambas plataformas para que los hits puedan compararse por id lado a lado.

La muestra de Flutter (`flutter/example`) refleja la misma UX, incluyendo los botones de añadir/eliminar por lote.

## Pruebas

Tres ejecutores — `cargo test` (Rust), `swift test` (Swift Testing) y `gradle :unifiedquery:test` (JUnit 5 en JVM)— ejecutan el mismo contrato de comportamiento contra el mismo núcleo de Rust. Cada flujo de CI se ejecuta independientemente y los tres deben permanecer en verde.

### Ubicación de cada tipo de prueba

La suite se divide en **cuatro capas**, un propósito por capa. La misma estratificación se reproduce en cada plataforma; cuando se añade una nueva plataforma, debe seguir exactamente la misma forma (ver *Añadir una nueva plataforma* más abajo).

| Capa | Reside en | Qué cubre | Qué **no** cubre |
|---|---|---|---|
| 1. Rust unitaria | `core/src/normalize/` & `core/src/engine.rs` (`#[cfg(test)] mod tests`) | Lógica interna del núcleo de Rust — tiene acceso a elementos privados. | Cualquier cosa que necesite la capa FFI. |
| 2. Basada en spec (multiplataforma) | `spec/*.json` + cargador por plataforma | Casos puros `(entrada → esperado)` y `(operaciones → ids)` compartidos por todos los ejecutores. Una deriva en el núcleo de Rust falla el **mismo `id`** en los tres CIs a la vez. | Aserciones de propiedad/desigualdad, smoke test de rendimiento, ciclo de vida del sistema de archivos, cordura de la puntuación; nada de esto se reduce a una igualdad simple sobre un valor. |
| 3. Ciclo de vida nativo | `*LifecycleTests` por plataforma | Apertura / reapertura / persistencia / ruta inválida en los tipos de I/O y error reales del lenguaje. | Comportamiento de búsqueda. |
| 4. Consulta nativa (no basada en datos) | `*QueryTests` / `*Tests` por plataforma | Ordenación bm25, respeto al `limit`, cordura de la puntuación (`0.0` para LIKE, finito-no-cero para FTS5), seguridad sin lanzamientos en especiales de FTS5, smoke test de concurrencia. | Cualquier cosa expresable como `(entrada → esperado)`, que pertenece en `spec/`. |

El principio: **si una aserción es una igualdad simple sobre un valor, ponla en `spec/`. Todo lo demás permanece en la suite nativa.** El README de la spec ([`spec/README.md`](spec/README.md)) mantiene la lista canónica de lo que está y no está en el alcance.

### Ejecución de pruebas

| Ejecutor | Comando | Qué carga |
|---|---|---|
| Rust unitaria | `cd core && cargo test --lib` | módulos `#[cfg(test)]` de `core/src/normalize/` & `core/src/engine.rs` |
| Rust conformidad | `cd core && cargo test --test conformance` | `core/tests/conformance.rs` → `../spec/*.json` |
| Rust (todo) | `cd core && cargo test --all-targets` | ambos anteriores (coincide con CI) |
| Swift Testing | `swift test` | `ios/Tests/UnifiedQueryTests/*.swift` (el `SpecLoader` sube desde `#filePath` para encontrar `spec/`) |
| JUnit 5 (JVM) | `cd android/sample && gradle :unifiedquery:test` | `unifiedquery/src/test/kotlin/.../*.kt` (obtiene `unfydqry.spec.dir` de `build.gradle.kts`) |

### El directorio `spec/`

`spec/normalize.json` y `spec/search.json` son la **única fuente de verdad para el comportamiento multiplataforma**. El esquema, las convenciones (versionado, `id`, `description`, reglas de alcance) y la intención están documentados en [`spec/README.md`](spec/README.md). En resumen:

- Cada archivo está versionado (`"version": 1`). Los cargadores se niegan a ejecutar si no coincide con la versión para la que fueron escritos; un cambio disruptivo en el esquema en el futuro no puede hacer que las pruebas pasen silenciosamente al no cargar nada.
- Cada caso lleva un `id` estable en snake-case y una `description` legible para humanos. Los cargadores deben incluir ambos en cada mensaje de fallo para que un log de CI solo sea suficiente para diagnosticar la rotura.
- `normalize.json` es una lista plana de casos `(input, expected)`.
- `search.json` tiene dos secciones: `scenarios` (una secuencia de `ops` seguidas de `assertions`) y `seeded_matrices` (una semilla compartida reutilizada en muchas consultas; más barato que sembrar por cada consulta).
- Las comparaciones de hit-id son **insensibles al orden** (igualdad de conjuntos). El orden solo se aserta en las suites de consultas nativas, contra bm25.

### Archivos de prueba por plataforma

Los archivos de prueba nativos (ciclo de vida + consulta) para cada binding se enumeran en su guía: iOS en [`docs/ios.md#tests`](docs/ios.md#tests), Android en [`docs/android.md#tests`](docs/android.md#tests). Ambos siguen la misma división de cuatro capas que el núcleo de Rust a continuación.

Rust (`core/`):

| Archivo | Capa | Notas |
|---|---|---|
| `src/normalize/mod.rs` `mod tests` | 1 — unitaria | Tabla de traza del doc de diseño §2.2; distinción dakuten/handakuten; `nfkc_case_fold` mantiene los kana distintos. |
| `src/engine.rs` `mod tests` | 1 — unitaria | Índice / eliminar / re-indexar / fallback LIKE / escape de comillas / consulta vacía; estrategias `prefix` & `substring`; `ConfigMismatch` en cambio de perfil; recuento de `reindex()` y regeneración de `withConfigRebuilding`. |
| `tests/conformance.rs` | 2 — basada en spec | Mismos `spec/*.json` que Swift y Kotlin, asertados directamente sobre la API de Rust en proceso. Detecta la deriva del núcleo independientemente de cualquier binding. |

La capa de consulta/ciclo de vida nativa **no** se refleja intencionalmente en las pruebas de integración de Rust; el núcleo de Rust no tiene un ciclo de vida específico de FFI que ejercitar (no hay `FileManager` de Swift, no hay cargador JNA), y las propiedades de bm25/ordenación están cubiertas por Swift+Kotlin que ejercitan el mismo camino de código.

### Añadir una nueva plataforma

Para implementar una nueva plataforma (ej. Python vía maturin, Node vía napi-rs, Flutter, Wasm/JS, .NET), la suite de pruebas debe mantener las mismas cuatro capas. Concretamente:

1. **Generar el binding UniFFI** para el nuevo lenguaje y commitarlo, siguiendo la misma convención que Swift / Kotlin (binding co-ubicado con el módulo de librería del lenguaje; lib nativa FFI cargada por la convención del lenguaje).
2. **Añadir un cargador de spec** para ese lenguaje. Debe:
   - Localizar el directorio `spec/` del repo (ya sea vía una propiedad del sistema de build como en el lado de Kotlin, o subiendo desde el archivo de prueba como en el lado de Swift, o vía una ruta relativa como en la prueba de integración de Rust).
   - Decodificar ambos archivos JSON en structs tipados que coincidan con [`spec/README.md`](spec/README.md) (`version`, `cases`, `scenarios`, `seeded_matrices`, `ops` como una unión etiquetada de `index`/`remove`).
   - Asertar `version == EXPECTED_VERSION` y negarse a ejecutar si no es así; esto es lo que evita que un futuro salto de esquema pase silenciosamente.
3. **Traducir las cuatro pruebas `Spec*`** (`normalize cases`, `scenarios`, `seeded_matrices`, más las dos comprobaciones de `version`) al idioma de pruebas parametrizadas del lenguaje. Cada caso debe mostrar `id` + `description` en el mensaje de fallo; esa es la pieza fundamental para la depuración entre CIs.
4. **Traducir las capas nativas** (ciclo de vida + consulta) siguiendo los pares Swift/Kotlin como plantillas. Están escritos deliberadamente como imágenes espejo uno del otro para que una tercera traducción sea sencilla. Mantenga el **límite de alcance** de la tabla anterior: cualquier cosa reducible a `(entrada → esperado)` pertenece en `spec/`, no aquí.
5. **Configurar un flujo de GitHub Actions** modelado en `.github/workflows/{swift,kotlin,rust}-tests.yml`. Las rutas de disparo deben incluir `core/**` y `spec/**` para que cualquier cambio en el núcleo o la spec vuelva a ejecutar también el CI de la nueva plataforma; esto es lo que hace que la deriva sea visible al mismo tiempo en todas las plataformas.
6. **Extender `spec/`, no las pruebas nativas, al añadir nueva cobertura de comportamiento** que todas las plataformas deban compartir. Un nuevo caso aterriza en el JSON una vez y se enciende en cada CI en la siguiente ejecución.

Un cambio que rompa el núcleo de Rust debe fallar con **el mismo `id` de caso** en cada plataforma simultáneamente. Si solo una plataforma falla en un caso de la spec, el cargador en esa plataforma está mal, no el núcleo.

## Lanzamiento (Releasing)

Dos flujos de lanzamiento residen en `.github/workflows/`:

| Artefacto | Flujo | Disparador | Publicado en |
|---|---|---|---|
| iOS XCFramework | `release-xcframework.yml` | manual (entrada de tag, ej. `v0.1.0`) | Activo de GitHub Release (`UnifiedQuery.xcframework.zip`) |
| Android AAR | `release-aar.yml` | tag de versión (`X.Y.Z`) o despacho manual | Maven Central (`:unifiedquery` AAR) |

Los procedimientos de lanzamiento paso a paso están en la guía de cada plataforma:

- iOS XCFramework — [`docs/ios.md#releasing-xcframework`](docs/ios.md#releasing-xcframework)
- Android AAR — [`docs/android.md#releasing-aar`](docs/android.md#releasing-aar)

## Mapa de Namespaces

| Capa | Nombre |
|---|---|
| Crate de Rust | `unfydqry` |
| Lib de Rust | `libunfydqry.{a,so,dylib}` |
| Namespace UniFFI | `unfydqry` |
| Módulo FFI de Swift | `unfydqryFFI` |
| Paquete SwiftPM | `UnifiedQuery` |
| Módulo Gradle de Android | `:unifiedquery` |
| Paquete de Kotlin | `uniffi.unfydqry` |

## Soporte avanzado de plataforma

Un **plugin de Flutter** envuelve los bindings nativos de iOS y Android detrás de una API de method-channel de Dart. Ahora reside en el árbol bajo `flutter/` (paquete Dart `unfydqry`) y su CI se ejecuta en `main`; vea la insignia de Flutter Tests arriba. La documentación completa está en [`docs/flutter-plugin.md`](docs/flutter-plugin.md).

| Runtime | Ubicación | Docs |
|---|---|---|
| Flutter | `flutter/` (paquete Dart `unfydqry`) | [`docs/flutter-plugin.md`](docs/flutter-plugin.md) |

El plugin **no** es parte de la distribución de iOS/Android: requiere que los artefactos nativos (XCFramework + `.so`) se construyan primero y está destinado a equipos que ya utilizan Flutter.

```sh
# Pruebas unitarias de Dart (mock method channel, no requiere artefactos nativos)
cd flutter && flutter test

# App de muestra (construya los artefactos nativos primero — ver docs/flutter-plugin.md)
cd flutter/example && flutter run
```

## Contribución

Humanos y agentes de IA trabajan en este repositorio en paralelo. El acuerdo de trabajo compartido que mantiene esto libre de colisiones y regresiones reside en [AGENTS.md](AGENTS.md); el recorrido de configuración está en [CONTRIBUTING.md](CONTRIBUTING.md). En resumen: los cambios de comportamiento van en `core/`, los bindings de Swift/Kotlin se generan (`make gen-bindings`, nunca se editan a mano), y `make ci` debe pasar antes de hacer push. Ejecute `make setup` una vez por clonación para habilitar los hooks del repo (`core.hooksPath` es configuración local y no se transporta por clonación/pull).

## Licencia

MIT — vea [LICENSE](LICENSE).
