mod pyrsutils;
use pyrsutils::prelude::*;

use iced::alignment::Vertical;
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Element, Fill, Font, Size, Task, Theme};
use std::fmt;
use std::path::{Path, PathBuf};

const APP_NAME: &str = "Prince of Persia - DAT/CFG Inspector";
const MAX_FILE_SIZE: u64 = 256 * 1024 * 1024;
const BYTES_PER_ROW: usize = 16;
const LINHAS_POR_PAGINA: usize = 400;
const MAX_LINHA: usize = 512;
const EXTENSOES: [&str; 8] = [".dat", ".cfg", ".exe", ".com", ".bat", ".dll", ".bin", ".wad"];
const ENCODINGS: [&str; 4] = ["cp437", "latin-1", "utf-8", "ascii"];

const CP437_ALTO: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αß\u{0393}π\u{03A3}σ\u{00B5}τ\u{03A6}\u{0398}\u{03A9}δ∞φε∩≡±≥≤⌠⌡÷≈°\u{2219}\u{00B7}√ⁿ²■\u{00A0}";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modo {
    Hex,
    Texto,
}

impl fmt::Display for Modo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Modo::Hex => write!(f, "HEX"),
            Modo::Texto => write!(f, "Texto"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Enc(usize);

impl fmt::Display for Enc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", ENCODINGS[self.0])
    }
}

static MODOS: [Modo; 2] = [Modo::Hex, Modo::Texto];
static ENCS: [Enc; 4] = [Enc(0), Enc(1), Enc(2), Enc(3)];

#[derive(Debug, Clone, Copy)]
enum Nav {
    Inicio,
    Anterior,
    Proxima,
    Fim,
}

#[derive(Debug, Clone)]
enum Msg {
    AbrirArquivo,
    ArquivoEscolhido(Option<PathBuf>),
    AbrirPasta,
    PastaEscolhida(Option<PathBuf>),
    Escolher(usize),
    CancelarLista,
    Info,
    Strings,
    BuscarTexto,
    BuscarHex,
    Query(String),
    Modo(Modo),
    Codificacao(Enc),
    Nav(Nav),
    FecharAviso,
}

enum Saida {
    Vazia,
    Hex,
    Linhas(Vec<String>),
}

struct App {
    path: Option<PathBuf>,
    data: Vec<u8>,
    modo: Modo,
    enc: usize,
    query: String,
    status: String,
    saida: Saida,
    ini: usize,
    lista: Option<Vec<(PathBuf, String)>>,
    aviso: Option<(String, String)>,
}

fn milhares(n: u64) -> String {
    let d = n.to_string();
    let mut r = String::with_capacity(d.len() + d.len() / 3);
    for (i, c) in d.chars().enumerate() {
        if i > 0 && (d.len() - i) % 3 == 0 {
            r.push(',');
        }
        r.push(c);
    }
    r
}

fn ext_ok(p: &Path) -> bool {
    let (_, ext) = splitext(p);
    EXTENSOES.contains(&lower(&ext).as_str())
}

fn decodificar(data: &[u8], enc: usize) -> String {
    match ENCODINGS[enc] {
        "cp437" => {
            let tabela: Vec<char> = CP437_ALTO.chars().collect();
            data.iter()
                .map(|&b| if b < 128 { b as char } else { tabela[(b - 128) as usize] })
                .collect()
        }
        "latin-1" => data.iter().map(|&b| b as char).collect(),
        "utf-8" => String::from_utf8_lossy(data).into_owned(),
        _ => data
            .iter()
            .map(|&b| if b < 128 { b as char } else { '\u{FFFD}' })
            .collect(),
    }
}

fn codificar(texto: &str, enc: usize) -> Result<Vec<u8>, String> {
    let nome = ENCODINGS[enc];
    if nome == "utf-8" {
        return Ok(texto.as_bytes().to_vec());
    }
    let tabela: Vec<char> = CP437_ALTO.chars().collect();
    let mut out = Vec::with_capacity(texto.len());
    for c in texto.chars() {
        let cp = c as u32;
        let byte = match nome {
            "cp437" => {
                if cp < 128 {
                    Some(cp as u8)
                } else {
                    tabela.iter().position(|&t| t == c).map(|i| (128 + i) as u8)
                }
            }
            "latin-1" => {
                if cp < 256 { Some(cp as u8) } else { None }
            }
            _ => {
                if cp < 128 { Some(cp as u8) } else { None }
            }
        };
        match byte {
            Some(b) => out.push(b),
            None => return Err(format!("'{nome}' codec can't encode character {c:?}")),
        }
    }
    Ok(out)
}

fn sanear(texto: &str) -> String {
    let mut s = String::with_capacity(texto.len());
    let mut it = texto.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\r' && it.peek() == Some(&'\n') {
            continue;
        }
        if c == '\n' {
            s.push(c);
        } else if c == '\t' {
            s.push_str("    ");
        } else if c.is_control() {
            s.push('\u{00B7}');
        } else {
            s.push(c);
        }
    }
    s
}

fn quebrar(texto: &str) -> Vec<String> {
    let mut v = Vec::new();
    for l in texto.split('\n') {
        if l.len() <= MAX_LINHA {
            v.push(l.to_string());
        } else {
            let cs: Vec<char> = l.chars().collect();
            for pedaco in cs.chunks(MAX_LINHA) {
                v.push(pedaco.iter().collect());
            }
        }
    }
    v
}

fn total_hex(data: &[u8]) -> usize {
    if data.is_empty() {
        3
    } else {
        2 + (data.len() + BYTES_PER_ROW - 1) / BYTES_PER_ROW
    }
}

fn linha_hex(data: &[u8], i: usize) -> String {
    if i == 0 {
        return "OFFSET    HEX BYTES                                      ASCII".to_string();
    }
    if i == 1 {
        return "-".repeat(78);
    }
    if data.is_empty() {
        return "(arquivo vazio)".to_string();
    }
    let offset = (i - 2) * BYTES_PER_ROW;
    let fim = (offset + BYTES_PER_ROW).min(data.len());
    let chunk = &data[offset..fim];
    let hex_part = join(" ", chunk.iter().map(|b| format!("{b:02X}")));
    let ascii_part: String = chunk
        .iter()
        .map(|&b| if (32..=126).contains(&b) { b as char } else { '.' })
        .collect();
    format!("{offset:08X}  {hex_part:<47}  {ascii_part}")
}

fn find_strings(data: &[u8], minimum: usize) -> Vec<(usize, String)> {
    let mut results = Vec::new();
    let mut start: Option<usize> = None;
    let mut buffer: Vec<u8> = Vec::new();

    for (offset, &byte) in data.iter().enumerate() {
        if (32..=126).contains(&byte) {
            if start.is_none() {
                start = Some(offset);
            }
            buffer.push(byte);
        } else {
            if let Some(s) = start {
                if buffer.len() >= minimum {
                    results.push((s, String::from_utf8_lossy(&buffer).into_owned()));
                }
            }
            start = None;
            buffer.clear();
        }
    }

    if let Some(s) = start {
        if buffer.len() >= minimum {
            results.push((s, String::from_utf8_lossy(&buffer).into_owned()));
        }
    }

    results
}

fn find_all(data: &[u8], needle: &[u8]) -> Vec<usize> {
    let mut offsets = Vec::new();
    let mut inicio = 0usize;
    while inicio + needle.len() <= data.len() {
        match data[inicio..]
            .windows(needle.len())
            .position(|w| w == needle)
        {
            Some(p) => {
                offsets.push(inicio + p);
                inicio += p + 1;
            }
            None => break,
        }
    }
    offsets
}

fn parse_hex(query: &str) -> Option<Vec<u8>> {
    let normalizado = query.replace("0x", "").replace("0X", "").replace(',', " ");
    let mut out = Vec::new();
    for tok in normalizado.split_whitespace() {
        if tok.len() % 2 != 0 || !tok.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        for i in (0..tok.len()).step_by(2) {
            out.push(u8::from_str_radix(&tok[i..i + 2], 16).ok()?);
        }
    }
    Some(out)
}

fn extensoes_dialogo() -> Vec<String> {
    let mut v = Vec::new();
    for e in EXTENSOES.iter() {
        let s = e.trim_start_matches('.');
        v.push(s.to_string());
        v.push(upper(s));
    }
    v
}

impl App {
    fn new() -> Self {
        App {
            path: None,
            data: Vec::new(),
            modo: Modo::Hex,
            enc: 0,
            query: String::new(),
            status: "Abra um arquivo (.DAT, .CFG, .EXE, .COM, .BAT, .DLL, .BIN, .WAD).".to_string(),
            saida: Saida::Vazia,
            ini: 0,
            lista: None,
            aviso: None,
        }
    }

    fn title(&self) -> String {
        APP_NAME.to_string()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn avisar(&mut self, titulo: &str, msg: impl Into<String>) {
        self.aviso = Some((titulo.to_string(), msg.into()));
    }

    fn set_output(&mut self, linhas: Vec<String>) {
        self.saida = Saida::Linhas(linhas);
        self.ini = 0;
        self.lista = None;
    }

    fn render(&mut self) {
        if self.path.is_none() {
            return;
        }
        self.lista = None;
        self.ini = 0;
        match self.modo {
            Modo::Hex => self.saida = Saida::Hex,
            Modo::Texto => {
                let texto = sanear(&decodificar(&self.data, self.enc));
                self.saida = Saida::Linhas(quebrar(&texto));
            }
        }
    }

    fn total(&self) -> usize {
        match &self.saida {
            Saida::Vazia => 0,
            Saida::Hex => total_hex(&self.data),
            Saida::Linhas(v) => v.len(),
        }
    }

    fn pagina_texto(&self) -> String {
        let total = self.total();
        let fim = (self.ini + LINHAS_POR_PAGINA).min(total);
        match &self.saida {
            Saida::Vazia => String::new(),
            Saida::Hex => join("\n", (self.ini..fim).map(|i| linha_hex(&self.data, i))),
            Saida::Linhas(v) => join("\n", v[self.ini..fim].iter()),
        }
    }

    fn load_file(&mut self, path: PathBuf) {
        self.lista = None;

        let size = match std::fs::metadata(&path) {
            Ok(m) => m.len(),
            Err(e) => {
                self.avisar("Erro de leitura", e.to_string());
                return;
            }
        };

        if size > MAX_FILE_SIZE {
            self.avisar("Arquivo muito grande", "O limite desta versão é 256 MiB.");
            return;
        }

        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                self.avisar("Erro de leitura", e.to_string());
                return;
            }
        };

        self.status = format!(
            "{} bytes carregados. Somente leitura.",
            milhares(len(&data) as u64)
        );
        self.path = Some(path);
        self.data = data;
        self.aviso = None;
        self.render();
    }

    fn listar_pasta(&mut self, pasta: PathBuf) {
        let rd = match std::fs::read_dir(&pasta) {
            Ok(rd) => rd,
            Err(e) => {
                self.avisar("Pasta", e.to_string());
                return;
            }
        };

        let files = sorted(
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_file() && ext_ok(p))
                .collect::<Vec<PathBuf>>(),
        );

        if files.is_empty() {
            self.avisar(
                "Arquivos",
                "Nenhum arquivo .DAT, .CFG, .EXE, .COM, .BAT, .DLL, .BIN ou .WAD encontrado.",
            );
            return;
        }

        let itens: Vec<(PathBuf, String)> = files
            .into_iter()
            .map(|p| {
                let nome = basename(&p);
                let rotulo = match p.metadata() {
                    Ok(m) => format!("{nome}  ({} bytes)", milhares(m.len())),
                    Err(_) => nome,
                };
                (p, rotulo)
            })
            .collect();

        self.lista = Some(itens);
        self.aviso = None;
    }

    fn show_info(&mut self) {
        let path = match &self.path {
            Some(p) => p.clone(),
            None => {
                self.avisar("Informações", "Abra um arquivo primeiro.");
                return;
            }
        };

        let data = &self.data;
        let printable = data
            .iter()
            .filter(|&&b| b == 9 || b == 10 || b == 13 || (32..=126).contains(&b))
            .count();
        let ratio = if data.is_empty() {
            0.0
        } else {
            printable as f64 / data.len() as f64 * 100.0
        };
        let mut vistos = [false; 256];
        for &b in data.iter() {
            vistos[b as usize] = true;
        }
        let distintos = vistos.iter().filter(|&&v| v).count();

        let info = format!(
            "Nome: {}\nCaminho: {}\nExtensão: {}\nTamanho: {} bytes\nBytes distintos: {}/256\nBytes ASCII imprimíveis/controles comuns: {:.1}%\n\nEssa proporção é apenas indicativa; não identifica sozinha o formato do arquivo.",
            basename(&path),
            path.display(),
            splitext(&path).1,
            milhares(data.len() as u64),
            distintos,
            ratio
        );

        self.avisar("Informações", info);
    }

    fn extract_strings(&mut self) {
        if self.path.is_none() {
            self.avisar("Strings", "Abra um arquivo primeiro.");
            return;
        }

        let results = find_strings(&self.data, 4);

        let mut lines = vec![
            format!("Strings ASCII encontradas: {}", results.len()),
            String::new(),
            "OFFSET    TEXTO".to_string(),
            "-".repeat(60),
        ];

        for (offset, value) in results.iter() {
            lines.push(format!("{offset:08X}  {value}"));
        }

        if results.is_empty() {
            lines.push("Nenhuma string encontrada.".to_string());
        }

        self.set_output(lines);
        self.status = format!("{} strings ASCII encontradas.", results.len());
    }

    fn search_text(&mut self) {
        if self.path.is_none() {
            self.avisar("Pesquisa", "Abra um arquivo primeiro.");
            return;
        }

        let query = self.query.clone();
        if query.is_empty() {
            return;
        }

        let needle = match codificar(&query, self.enc) {
            Ok(n) => n,
            Err(m) => {
                self.avisar("Codificação", m);
                return;
            }
        };

        if needle.is_empty() {
            return;
        }

        let offsets = find_all(&self.data, &needle);
        self.show_search_results(&format!("Pesquisa textual: {query:?}"), &needle, &offsets);
    }

    fn search_hex(&mut self) {
        if self.path.is_none() {
            self.avisar("Pesquisa HEX", "Abra um arquivo primeiro.");
            return;
        }

        let query = self.query.clone();

        let needle = match parse_hex(strip(&query)) {
            Some(n) => n,
            None => {
                self.avisar("HEX inválido", "Use, por exemplo: 50 52 49 4E 43 45");
                return;
            }
        };

        if needle.is_empty() {
            return;
        }

        let offsets = find_all(&self.data, &needle);
        self.show_search_results("Pesquisa hexadecimal", &needle, &offsets);
    }

    fn show_search_results(&mut self, title: &str, needle: &[u8], offsets: &[usize]) {
        let mut lines = vec![
            title.to_string(),
            format!(
                "Bytes: {}",
                join(" ", needle.iter().map(|b| format!("{b:02X}")))
            ),
            format!("Ocorrências: {}", offsets.len()),
            String::new(),
        ];

        for &offset in offsets {
            lines.push(format!(
                "Decimal: {:>10} | HEX: 0x{offset:08X}",
                milhares(offset as u64)
            ));
        }

        if offsets.is_empty() {
            lines.push("Nenhuma ocorrência encontrada.".to_string());
        }

        self.set_output(lines);
        self.status = format!("{} ocorrência(s) encontrada(s).", offsets.len());
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::AbrirArquivo => {
                let exts = extensoes_dialogo();
                return Task::perform(
                    async move {
                        rfd::AsyncFileDialog::new()
                            .set_title("Abrir arquivo do jogo")
                            .add_filter("Arquivos suportados", exts.as_slice())
                            .pick_file()
                            .await
                            .map(|h| h.path().to_path_buf())
                    },
                    Msg::ArquivoEscolhido,
                );
            }
            Msg::ArquivoEscolhido(Some(p)) => self.load_file(p),
            Msg::ArquivoEscolhido(None) => {}
            Msg::AbrirPasta => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Selecionar pasta do jogo")
                            .pick_folder()
                            .await
                            .map(|h| h.path().to_path_buf())
                    },
                    Msg::PastaEscolhida,
                );
            }
            Msg::PastaEscolhida(Some(p)) => self.listar_pasta(p),
            Msg::PastaEscolhida(None) => {}
            Msg::Escolher(i) => {
                let alvo = self
                    .lista
                    .as_ref()
                    .and_then(|l| l.get(i))
                    .map(|(p, _)| p.clone());
                if let Some(p) = alvo {
                    self.load_file(p);
                }
            }
            Msg::CancelarLista => self.lista = None,
            Msg::Info => self.show_info(),
            Msg::Strings => self.extract_strings(),
            Msg::BuscarTexto => self.search_text(),
            Msg::BuscarHex => self.search_hex(),
            Msg::Query(q) => self.query = q,
            Msg::Modo(m) => {
                self.modo = m;
                self.render();
            }
            Msg::Codificacao(e) => {
                self.enc = e.0;
                self.render();
            }
            Msg::Nav(n) => {
                let total = self.total();
                let max_ini = if total == 0 {
                    0
                } else {
                    ((total - 1) / LINHAS_POR_PAGINA) * LINHAS_POR_PAGINA
                };
                self.ini = match n {
                    Nav::Inicio => 0,
                    Nav::Anterior => self.ini.saturating_sub(LINHAS_POR_PAGINA),
                    Nav::Proxima => (self.ini + LINHAS_POR_PAGINA).min(max_ini),
                    Nav::Fim => max_ini,
                };
            }
            Msg::FecharAviso => self.aviso = None,
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Msg> {
        let caminho = match &self.path {
            Some(p) => p.display().to_string(),
            None => "Nenhum arquivo aberto".to_string(),
        };

        let toolbar = container(
            row![
                button("Abrir arquivo").on_press(Msg::AbrirArquivo),
                button("Abrir pasta").on_press(Msg::AbrirPasta),
                button("Informações").on_press(Msg::Info),
                button("Extrair strings").on_press(Msg::Strings),
                text("Visualização:"),
                pick_list(&MODOS[..], Some(self.modo), Msg::Modo),
                text(caminho).width(Fill),
            ]
            .spacing(8)
            .align_y(Vertical::Center),
        )
        .padding(8)
        .width(Fill)
        .style(container::bordered_box);

        let busca = container(
            row![
                text_input("Texto ou HEX: PRINCE / 50 52 49 4E 43 45", &self.query)
                    .on_input(Msg::Query)
                    .on_submit(Msg::BuscarTexto)
                    .width(Fill),
                button("Buscar texto").on_press(Msg::BuscarTexto),
                button("Buscar HEX").on_press(Msg::BuscarHex),
            ]
            .spacing(8)
            .align_y(Vertical::Center),
        )
        .padding(8)
        .width(Fill)
        .style(container::bordered_box);

        let banner: Option<Element<'_, Msg>> = self.aviso.as_ref().map(|(t, m)| {
            container(
                column![
                    text(t.as_str()).size(18),
                    text(m.as_str()),
                    button("OK").on_press(Msg::FecharAviso),
                ]
                .spacing(6),
            )
            .padding(10)
            .width(Fill)
            .style(container::bordered_box)
            .into()
        });

        let conteudo: Element<'_, Msg> = if let Some(lista) = &self.lista {
            column![
                text("Arquivos encontrados").size(18),
                scrollable(
                    column(lista.iter().enumerate().map(|(i, (_, rotulo))| {
                        button(text(rotulo.as_str()).font(Font::MONOSPACE))
                            .on_press(Msg::Escolher(i))
                            .width(Fill)
                            .into()
                    }))
                    .spacing(4)
                )
                .width(Fill)
                .height(Fill),
                button("Cancelar").on_press(Msg::CancelarLista),
            ]
            .spacing(8)
            .height(Fill)
            .into()
        } else {
            let total = self.total();
            let fim = (self.ini + LINHAS_POR_PAGINA).min(total);

            let corpo = scrollable(
                text(self.pagina_texto())
                    .font(Font::MONOSPACE)
                    .size(13)
                    .wrapping(text::Wrapping::None),
            )
            .direction(scrollable::Direction::Both {
                vertical: scrollable::Scrollbar::default(),
                horizontal: scrollable::Scrollbar::default(),
            })
            .width(Fill)
            .height(Fill);

            let corpo = container(corpo)
                .padding(6)
                .width(Fill)
                .height(Fill)
                .style(container::bordered_box);

            let mut coluna = column![corpo].spacing(6).height(Fill);

            if total > LINHAS_POR_PAGINA {
                let nav: Element<'_, Msg> = row![
                    button("|<").on_press(Msg::Nav(Nav::Inicio)),
                    button("<").on_press(Msg::Nav(Nav::Anterior)),
                    button(">").on_press(Msg::Nav(Nav::Proxima)),
                    button(">|").on_press(Msg::Nav(Nav::Fim)),
                    text(format!(
                        "Linhas {}-{} de {}",
                        milhares((self.ini + 1) as u64),
                        milhares(fim as u64),
                        milhares(total as u64)
                    )),
                ]
                .spacing(8)
                .align_y(Vertical::Center)
                .into();
                coluna = coluna.push(nav);
            }

            coluna.into()
        };

        let rodape = container(
            row![
                text(&self.status).width(Fill),
                text("Codificação:"),
                pick_list(&ENCS[..], Some(Enc(self.enc)), Msg::Codificacao),
            ]
            .spacing(8)
            .align_y(Vertical::Center),
        )
        .padding(8)
        .width(Fill)
        .style(container::bordered_box);

        let mut principal = column![toolbar, busca].spacing(8).padding(10);

        if let Some(b) = banner {
            principal = principal.push(b);
        }

        principal.push(conteudo).push(rodape).into()
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .window(iced::window::Settings {
            size: Size::new(1150.0, 750.0),
            min_size: Some(Size::new(800.0, 500.0)),
            ..Default::default()
        })
        .run()
}
