mod pyrsutils;
use pyrsutils::prelude::*;

use std::path::{Path, PathBuf};

const APP_NAME: &str = "Prince of Persia - DAT/CFG Inspector";
const MAX_FILE_SIZE: u64 = 256 * 1024 * 1024;
const BYTES_PER_ROW: usize = 16;
const LINHAS_POR_PAGINA: usize = 30;
const EXTENSOES: [&str; 8] = [".dat", ".cfg", ".exe", ".com", ".bat", ".dll", ".bin", ".wad"];
const ENCODINGS: [&str; 4] = ["cp437", "latin-1", "utf-8", "ascii"];

const CP437_ALTO: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αß\u{0393}π\u{03A3}σ\u{00B5}τ\u{03A6}\u{0398}\u{03A9}δ∞φε∩≡±≥≤⌠⌡÷≈°\u{2219}\u{00B7}√ⁿ²■\u{00A0}";

enum Saida {
    Vazia,
    Hex,
    Linhas(Vec<String>),
}

struct App {
    path: Option<PathBuf>,
    data: Vec<u8>,
    hex: bool,
    enc: usize,
    query: String,
    status: String,
    saida: Saida,
}

fn ler(prompt: &str) -> String {
    match try_input::<String>(prompt) {
        Ok(s) => s,
        Err(_) => {
            print("");
            std::process::exit(0);
        }
    }
}

fn aviso(titulo: &str, msg: &str) {
    print(format!("[{titulo}] {msg}"));
}

fn erro(titulo: &str, msg: &str) {
    print(format!("[ERRO] {titulo}: {msg}"));
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

fn limpar_caminho(entrada: &str) -> String {
    let s = strip(entrada).trim_matches(|c| c == '"' || c == '\'');
    expanduser(strip(s))
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
            None => {
                return Err(format!(
                    "'{nome}' codec can't encode character {c:?}"
                ))
            }
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
        if c == '\n' || c == '\t' {
            s.push(c);
        } else if c.is_control() {
            s.push('\u{00B7}');
        } else {
            s.push(c);
        }
    }
    s
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

fn paginar(total: usize, linha: &dyn Fn(usize) -> String) {
    if total == 0 {
        return;
    }
    let mut ini = 0usize;
    loop {
        let fim = (ini + LINHAS_POR_PAGINA).min(total);
        for i in ini..fim {
            print(linha(i));
        }
        let resp = ler(&format!(
            "-- linhas {}-{} de {} -- [Enter]=próxima  [a]=anterior  [q]=sair: ",
            ini + 1,
            fim,
            total
        ));
        match lower(strip(&resp)).as_str() {
            "q" => return,
            "a" => ini = ini.saturating_sub(LINHAS_POR_PAGINA),
            _ => {
                if fim >= total {
                    return;
                }
                ini = fim;
            }
        }
    }
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

impl App {
    fn new() -> Self {
        App {
            path: None,
            data: Vec::new(),
            hex: true,
            enc: 0,
            query: String::new(),
            status: "Abra um arquivo (.DAT, .CFG, .EXE, .COM, .BAT, .DLL, .BIN, .WAD).".to_string(),
            saida: Saida::Vazia,
        }
    }

    fn mostrar(&self) {
        match &self.saida {
            Saida::Vazia => print("(sem saída)"),
            Saida::Hex => {
                let d = &self.data;
                paginar(total_hex(d), &|i| linha_hex(d, i));
            }
            Saida::Linhas(v) => paginar(v.len(), &|i| v[i].clone()),
        }
    }

    fn set_output(&mut self, linhas: Vec<String>) {
        self.saida = Saida::Linhas(linhas);
    }

    fn render(&mut self) {
        if self.path.is_none() {
            return;
        }
        if self.hex {
            self.saida = Saida::Hex;
        } else {
            let texto = sanear(&decodificar(&self.data, self.enc));
            self.saida = Saida::Linhas(texto.split('\n').map(String::from).collect());
        }
        self.mostrar();
    }

    fn cabecalho(&self) {
        let caminho = match &self.path {
            Some(p) => p.display().to_string(),
            None => "Nenhum arquivo aberto".to_string(),
        };
        print("");
        print("=".repeat(70));
        print(APP_NAME);
        print("=".repeat(70));
        print(format!("Arquivo      : {caminho}"));
        print(format!(
            "Visualização : {}    Codificação: {}",
            if self.hex { "HEX" } else { "Texto" },
            ENCODINGS[self.enc]
        ));
        print(format!("Status       : {}", self.status));
        print("-".repeat(70));
        print("1) Abrir arquivo        6) Codificação");
        print("2) Abrir pasta          7) Buscar texto");
        print("3) Informações          8) Buscar HEX");
        print("4) Extrair strings      9) Exibir saída atual");
        print("5) Visualização         0) Sair");
    }

    fn open_file(&mut self) {
        let entrada = ler("Caminho do arquivo (Enter cancela): ");
        let caminho = limpar_caminho(&entrada);
        if caminho.is_empty() {
            return;
        }
        self.load_file(PathBuf::from(caminho));
    }

    fn open_folder(&mut self) {
        let entrada = ler("Pasta do jogo (Enter cancela): ");
        let pasta = limpar_caminho(&entrada);
        if pasta.is_empty() {
            return;
        }

        let lista: Vec<PathBuf> = match std::fs::read_dir(&pasta) {
            Ok(rd) => rd
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_file() && ext_ok(p))
                .collect(),
            Err(e) => {
                erro("Pasta", &e.to_string());
                return;
            }
        };
        let files = sorted(lista);

        if files.is_empty() {
            aviso(
                "Arquivos",
                "Nenhum arquivo .DAT, .CFG, .EXE, .COM, .BAT, .DLL, .BIN ou .WAD encontrado.",
            );
            return;
        }

        print("");
        print("Arquivos encontrados:");
        for (i, p) in enumerate(files.iter()) {
            let nome = basename(p);
            match p.metadata() {
                Ok(m) => print(format!("{:>4}) {nome}  ({} bytes)", i + 1, milhares(m.len()))),
                Err(_) => print(format!("{:>4}) {nome}", i + 1)),
            }
        }

        let escolha = ler("Número do arquivo a abrir (Enter cancela): ");
        let escolha = strip(&escolha);
        if escolha.is_empty() {
            return;
        }
        match escolha.parse::<usize>() {
            Ok(n) if n >= 1 && n <= len(&files) => {
                self.load_file(files[n - 1].clone());
            }
            _ => aviso("Arquivos", "Seleção inválida."),
        }
    }

    fn load_file(&mut self, path: PathBuf) {
        let size = match std::fs::metadata(&path) {
            Ok(m) => m.len(),
            Err(e) => {
                erro("Erro de leitura", &e.to_string());
                return;
            }
        };

        if size > MAX_FILE_SIZE {
            erro("Arquivo muito grande", "O limite desta versão é 256 MiB.");
            return;
        }

        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                erro("Erro de leitura", &e.to_string());
                return;
            }
        };

        self.status = format!(
            "{} bytes carregados. Somente leitura.",
            milhares(len(&data) as u64)
        );
        self.path = Some(path);
        self.data = data;
        self.render();
    }

    fn show_info(&self) {
        let path = match &self.path {
            Some(p) => p,
            None => {
                aviso("Informações", "Abra um arquivo primeiro.");
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

        print("");
        print(format!("Nome: {}", basename(path)));
        print(format!("Caminho: {}", path.display()));
        print(format!("Extensão: {}", splitext(path).1));
        print(format!("Tamanho: {} bytes", milhares(data.len() as u64)));
        print(format!("Bytes distintos: {distintos}/256"));
        print(format!(
            "Bytes ASCII imprimíveis/controles comuns: {ratio:.1}%"
        ));
        print("");
        print("Essa proporção é apenas indicativa; não identifica sozinha o formato do arquivo.");
    }

    fn extract_strings(&mut self) {
        if self.path.is_none() {
            aviso("Strings", "Abra um arquivo primeiro.");
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
        self.mostrar();
    }

    fn pedir_consulta(&mut self, rotulo: &str) -> String {
        let msg = if self.query.is_empty() {
            format!("{rotulo}: ")
        } else {
            format!("{rotulo} [{}]: ", self.query)
        };
        let entrada = ler(&msg);
        if !entrada.is_empty() {
            self.query = entrada;
        }
        self.query.clone()
    }

    fn search_text(&mut self) {
        if self.path.is_none() {
            aviso("Pesquisa", "Abra um arquivo primeiro.");
            return;
        }

        let query = self.pedir_consulta("Texto a buscar (ex.: PRINCE)");
        if query.is_empty() {
            return;
        }

        let needle = match codificar(&query, self.enc) {
            Ok(n) => n,
            Err(m) => {
                erro("Codificação", &m);
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
            aviso("Pesquisa HEX", "Abra um arquivo primeiro.");
            return;
        }

        let bruto = self.pedir_consulta("HEX a buscar (ex.: 50 52 49 4E 43 45)");
        let query = strip(&bruto);

        let needle = match parse_hex(query) {
            Some(n) => n,
            None => {
                erro("HEX inválido", "Use, por exemplo: 50 52 49 4E 43 45");
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
        self.mostrar();
    }

    fn escolher_modo(&mut self) {
        print("1) HEX");
        print("2) Texto");
        let e = ler("Visualização: ");
        match strip(&e) {
            "1" => self.hex = true,
            "2" => self.hex = false,
            _ => {
                aviso("Visualização", "Opção inválida.");
                return;
            }
        }
        self.render();
    }

    fn escolher_codificacao(&mut self) {
        for (i, nome) in enumerate(ENCODINGS.iter()) {
            print(format!("{}) {nome}", i + 1));
        }
        let e = ler("Codificação: ");
        match strip(&e).parse::<usize>() {
            Ok(n) if n >= 1 && n <= ENCODINGS.len() => self.enc = n - 1,
            _ => {
                aviso("Codificação", "Opção inválida.");
                return;
            }
        }
        self.render();
    }

    fn run(&mut self) {
        loop {
            self.cabecalho();
            let op = ler("Opção: ");
            match lower(strip(&op)).as_str() {
                "1" => self.open_file(),
                "2" => self.open_folder(),
                "3" => self.show_info(),
                "4" => self.extract_strings(),
                "5" => self.escolher_modo(),
                "6" => self.escolher_codificacao(),
                "7" => self.search_text(),
                "8" => self.search_hex(),
                "9" => self.mostrar(),
                "0" | "q" | "sair" => break,
                _ => aviso("Menu", "Opção inválida."),
            }
        }
    }
}

fn main() {
    let mut app = App::new();
    app.run();
}