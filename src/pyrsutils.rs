//! # pyrsutils — v1.4.0
//!
//! Biblioteca única de utilidades (zero dependências externas, 100% `std`) que
//! traz para o Rust a ergonomia do dia a dia de Python (e, na seção de
//! "classes", a sintaxe familiar de C++), sem abrir mão de nenhuma garantia do
//! Rust: tipagem estática, checagem em tempo de compilação, *zero-cost
//! abstractions*, propriedade de memória (*ownership*) segura, sem coletor de
//! lixo (GC) e sem custo oculto em tempo de execução. Tudo aqui utiliza
//! **monomorphization** (código especializado por tipo em tempo de compilação)
//! — nunca *dynamic dispatch* (`dyn Any`) escondido.
//!
//! ## O que mudou na v1.4.0 (Reconstrução Sênior)
//!
//! 1. **Higiene total de macros (`$crate`) e compatibilidade entre Editions:**
//!    `input!`, `try_input!` e `timeit!` agora resolvem caminhos via `$crate::`,
//!    e `TryInto` é importado explicitamente para compilar limpo até mesmo se
//!    invocado com `rustc` direto sem `--edition=2021`.
//! 2. **`arithmetic::pow` em $O(\log N)$ e `usize`/`isize`:** Substituído o laço
//!    linear por exponenciação rápida por quadratura; adicionados `usize`,
//!    `isize`, `abs` e `divmod` seguro; corrigido o domínio matemático de
//!    `root(x, indice)` para índices fracionários e zero/NaN.
//! 3. **`string_utils::join` com alocação única + novos utilitários:** Eliminado
//!    o `Vec<String>` intermediário no `join`. Adicionados `replace`, `lower`,
//!    `upper`, `startswith`, `endswith`, `find`, `count` e `is_palindrome`.
//! 4. **Segurança e performance em `fs_utils::walk` + I/O direto:** `Walk` agora
//!    usa `DirEntry::file_type()` (não segue links simbólicos, evitando loops
//!    infinitos e poupando chamadas `stat` ao kernel). Adicionados `read_text`,
//!    `readlines`, `write_text` e `append_text`.
//! 5. **Built-ins expandidos:** `PyLen` agora cobre `String`, `Vec`, `VecDeque`,
//!    `HashSet`, `BTreeMap` e `BTreeSet`. Novas funções: `print`, `enumerate`,
//!    `zip`, `any`, `all` e `histogram` (contagem de frequências estilo Python).
//! 6. **Multiplicação de matrizes otimizada para cache (`Matrix::dot`):**
//!    Transpõe a segunda matriz internamente para garantir leitura sequencial
//!    alinhada na memória contígua.
//! 7. **Macro `class!` com suporte a Herança (`struct Filha : Pai`):**
//!    Implementa relação *IS-A* / *HAS-A* via composição (`pub base: Pai`) +
//!    `Deref`/`DerefMut` com custo zero em tempo de execução.
//! 8. **Novo módulo `random_utils` (Seção 11):** Equivalente ao módulo `random`
//!    do Python (`random`, `randint`, `choice`, `shuffle`, `seed`) usando
//!    SplitMix64 + Xorshift64* em `std` pura.
//!
//! ## Tabela Geral — pyrsutils ↔ Python ↔ C++
//!
//! | Seção      | Python                                      | C++ / Rust nativo                  | Aqui (`pyrsutils`)                                      |
//! |------------|---------------------------------------------|------------------------------------|---------------------------------------------------------|
//! | Entrada    | `input()`                                   | `std::cin >>`                      | `input::<T>(prompt)` / `input!()` / `try_input()`       |
//! | Ranges     | `range(a, b, c)`                            | `for(int i=a;i<b;i+=c)`            | `range` / `range_from` / `range_step`                   |
//! | Listas     | `append/pop/remove/extend/index`            | `std::vector::push_back`           | `Vec<T>` + trait `PyList`                               |
//! | Matriz     | `[[0]*c for _ in range(l)]`                 | `std::vector<vector<T>>`           | `Matrix<T>` (contígua, `transpose`, `dot`)              |
//! | Built-ins  | `print/len/min/max/sorted/enumerate/zip`    | `.size()/std::min/std::sort`       | `builtins::{print,len,min,max,sorted,reversed,zip,...}` |
//! | Aritmética | `sum/math.prod/pow/divmod/abs`              | operadores nativos                 | `arithmetic::{sum,sub,mult,div,divmod,pow,root,abs}`    |
//! | Strings    | `split/strip/join/replace/find/count`       | `std::string`                      | `string_utils::{split,strip,join,replace,find,count}`   |
//! | Classe     | `class Y(X): def __init__(self):`           | `class Y : public X { ... };`      | `class! { struct Y : X {...} impl {...} }`              |
//! | Tempo      | `time/sleep/perf_counter/timeit/strftime`   | `std::chrono` / `clock_gettime`    | `time_utils::{time,sleep,perf_counter,timeit,strftime}` |
//! | Rede       | `socket` (TCP/UDP/DNS)                      | `socket()/bind()/accept()`         | `net_utils::{TcpClient,TcpServer,UdpSock,getaddrinfo}`  |
//! | Arquivos   | `os/os.path/shutil/open().read()`           | `std::filesystem` / `std::fstream` | `fs_utils::{exists,listdir,walk,glob,read_text,...}`    |
//! | Aleatório  | `random.random/randint/choice/shuffle/seed` | `<random>` (`std::mt19937_64`)     | `random_utils::{random,randint,choice,shuffle,seed}`    |
//!
//! ## Uso Rápido
//! ```rust
//! use pyrsutils::prelude::*;
//! ```

#![allow(unused)]

use std::fmt::{Debug, Display};
use std::io::{self, Write};
use std::str::FromStr;

// ============================================================================
// 0) input() / try_input() — equivalente unificado e genérico ao input()
//    do Python, para qualquer T: FromStr (String, i32, f64, bool, char...)
// ============================================================================

/// Erro unificado de leitura e conversão de entrada padrão (`stdin`).
#[derive(Debug)]
pub enum InputError<E> {
    /// Stream fechado antes de qualquer dado (Ctrl+D no Linux / Ctrl+Z no Windows).
    Eof,
    /// Falha de I/O ao ler `stdin` ou descarregar `stdout`.
    Io(io::Error),
    /// O texto lido não pôde ser convertido para o tipo `T` esperado.
    Parse(E),
}

impl<E: Display> Display for InputError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputError::Eof => write!(f, "EOFError: fim da entrada (Ctrl+D/Ctrl+Z)"),
            InputError::Io(e) => write!(f, "IOError: falha ao ler entrada ({e})"),
            InputError::Parse(e) => write!(f, "ValueError: conversão falhou ({e})"),
        }
    }
}

impl<E: Debug + Display> std::error::Error for InputError<E> {}

/// Lê uma linha crua de `stdin` removendo apenas `\n` ou `\r\n` final.
fn read_raw(prompt: &str) -> Result<String, io::Error> {
    if !prompt.is_empty() {
        print!("{prompt}");
        io::stdout().flush()?;
    }

    let mut buffer = String::new();
    let bytes_read = io::stdin().read_line(&mut buffer)?;

    if bytes_read == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "EOF"));
    }

    if buffer.ends_with('\n') {
        buffer.pop();
        if buffer.ends_with('\r') {
            buffer.pop();
        }
    }

    Ok(buffer)
}

/// Equivalente unificado ao `input()` (e `int(input())`, `float(input())`) do Python.
///
/// O tipo de retorno `T` é inferido automaticamente pelo compilador a partir
/// da variável ou função onde o resultado é usado. Se o usuário digitar um
/// valor numérico com espaços nas pontas (ex.: `" 42 "`), para tipos diferentes
/// de `String` o texto passa por `.trim()` automaticamente (igual ao `int(" 42 ")`
/// do Python) caso a conversão direta falhe na primeira tentativa.
///
/// # Panics
/// Encerra com `panic!` em caso de EOF ou falha de conversão (mesmo comportamento
/// de um script Python sem bloco `try/except`). Para tratar erros sem panic,
/// use [`try_input`].
pub fn input<T>(prompt: &str) -> T
where
    T: FromStr,
    T::Err: Debug,
{
    match try_input::<T>(prompt) {
        Ok(valor) => valor,
        Err(InputError::Eof) => panic!("EOFError: entrada finalizada (Ctrl+D/Ctrl+Z)"),
        Err(InputError::Io(e)) => panic!("IOError: erro ao ler stdin: {e}"),
        Err(InputError::Parse(e)) => {
            panic!("ValueError: entrada inválida para o tipo esperado ({e:?})")
        }
    }
}

/// Versão segura de [`input`] que devolve `Result<T, InputError<T::Err>>` em
/// vez de causar `panic!`. Equivalente ao bloco `try: ... except ValueError:`
/// do Python.
pub fn try_input<T>(prompt: &str) -> Result<T, InputError<T::Err>>
where
    T: FromStr,
{
    let texto = read_raw(prompt).map_err(|e| {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            InputError::Eof
        } else {
            InputError::Io(e)
        }
    })?;

    // Tenta o parse exato primeiro (preserva espaços se T = String);
    // se falhar (ex.: " 42 " para i32), tenta com trim() igual ao int() do Python.
    match texto.parse::<T>() {
        Ok(v) => Ok(v),
        Err(_) => texto.trim().parse::<T>().map_err(InputError::Parse),
    }
}

/// Macro ergonômica: `input!()` ou `input!("Digite: ")`.
/// Usa `$crate::input` para funcionar em qualquer módulo sem imports extras.
#[macro_export]
macro_rules! input {
    () => {
        $crate::input("")
    };
    ($prompt:expr) => {
        $crate::input($prompt)
    };
}

/// Macro ergonômica: `try_input!()` ou `try_input!("Digite: ")`.
#[macro_export]
macro_rules! try_input {
    () => {
        $crate::try_input("")
    };
    ($prompt:expr) => {
        $crate::try_input($prompt)
    };
}

// ============================================================================
// 1) range() — sequências lazy sem alocar memória (Iterator O(1) em espaço)
// ============================================================================
pub mod range_utils {
    //! Equivalente ao `range()` do Python para inteiros com sinal (`i64`).
    //!
    //! | Python                     | Aqui                            |
    //! |----------------------------|---------------------------------|
    //! | `range(stop)`              | `range(stop)`                   |
    //! | `range(start, stop)`       | `range_from(start, stop)`       |
    //! | `range(start, stop, step)` | `range_step(start, stop, step)` |

    use super::builtins::PyLen;
    use std::convert::TryInto;

    /// Iterador preguiçoso equivalente ao `range` do Python (suporta passo negativo).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PyRange {
        atual: i64,
        fim: i64,
        passo: i64,
    }

    impl PyRange {
        /// Verifica em $O(1)$ se um valor `x` pertence à sequência (`x in range(...)`).
        #[inline]
        pub fn contains(&self, x: i64) -> bool {
            if self.passo > 0 {
                x >= self.atual && x < self.fim && (x - self.atual) % self.passo == 0
            } else {
                x <= self.atual && x > self.fim && (self.atual - x) % (-self.passo) == 0
            }
        }
    }

    impl Iterator for PyRange {
        type Item = i64;

        #[inline]
        fn next(&mut self) -> Option<i64> {
            if self.passo > 0 {
                if self.atual >= self.fim {
                    return None;
                }
            } else if self.atual <= self.fim {
                return None;
            }
            let valor = self.atual;
            self.atual += self.passo;
            Some(valor)
        }

        #[inline]
        fn size_hint(&self) -> (usize, Option<usize>) {
            let n = ExactSizeIterator::len(self);
            (n, Some(n))
        }
    }

    impl ExactSizeIterator for PyRange {
        #[inline]
        fn len(&self) -> usize {
            let (atual, fim, passo) = (self.atual as i128, self.fim as i128, self.passo as i128);
            let n = if passo > 0 {
                if atual >= fim {
                    0
                } else {
                    (fim - atual + passo - 1) / passo
                }
            } else if atual <= fim {
                0
            } else {
                let passo_abs = -passo;
                (atual - fim + passo_abs - 1) / passo_abs
            };
            n.try_into().unwrap_or(usize::MAX)
        }
    }

    impl PyLen for PyRange {
        #[inline]
        fn py_len(&self) -> usize {
            ExactSizeIterator::len(self)
        }
    }

    /// `range(stop)` — gera `0, 1, ..., stop - 1`.
    #[inline]
    pub fn range(stop: i64) -> PyRange {
        PyRange {
            atual: 0,
            fim: stop,
            passo: 1,
        }
    }

    /// `range_from(start, stop)` — gera `start, start + 1, ..., stop - 1`.
    #[inline]
    pub fn range_from(start: i64, stop: i64) -> PyRange {
        PyRange {
            atual: start,
            fim: stop,
            passo: 1,
        }
    }

    /// `range_step(start, stop, step)` — suporta passo positivo ou negativo.
    ///
    /// # Panics
    /// Causa panic se `step == 0` (`ValueError` no Python).
    #[inline]
    pub fn range_step(start: i64, stop: i64, step: i64) -> PyRange {
        assert!(step != 0, "ValueError: range() com step igual a zero");
        PyRange {
            atual: start,
            fim: stop,
            passo: step,
        }
    }
}

// ============================================================================
// 2) Listas — métodos estilo Python sobre Vec<T>, via Extension Trait
// ============================================================================
pub mod list_utils {
    //! Métodos ergonômicos estilo Python sobre `Vec<T>` com custo zero.

    pub trait PyList<T> {
        /// Adiciona um item ao final — equivalente a `lista.append(x)`.
        fn append_item(&mut self, valor: T);

        /// Estende a lista com qualquer iterável — equivalente a `lista.extend(iter)`.
        fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I);

        /// Remove e devolve o item no índice dado, ou o último se `None` —
        /// equivalente a `lista.pop()` / `lista.pop(i)`.
        fn pop_at(&mut self, indice: Option<usize>) -> Option<T>;

        /// Remove a primeira ocorrência igual a `valor` — equivalente a `lista.remove(x)`.
        /// Devolve `true` se encontrou e removeu.
        fn remove_value(&mut self, valor: &T) -> bool
        where
            T: PartialEq;

        /// Devolve o índice da primeira ocorrência de `valor` — equivalente a `lista.index(x)`.
        fn index_of(&self, valor: &T) -> Option<usize>
        where
            T: PartialEq;

        /// Conta quantas vezes `valor` aparece na lista — equivalente a `lista.count(x)`.
        fn count_value(&self, valor: &T) -> usize
        where
            T: PartialEq;
    }

    impl<T> PyList<T> for Vec<T> {
        #[inline]
        fn append_item(&mut self, valor: T) {
            self.push(valor);
        }

        #[inline]
        fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
            self.extend(iter);
        }

        #[inline]
        fn pop_at(&mut self, indice: Option<usize>) -> Option<T> {
            match indice {
                None => self.pop(),
                Some(i) if i < self.len() => Some(self.remove(i)),
                Some(_) => None,
            }
        }

        fn remove_value(&mut self, valor: &T) -> bool
        where
            T: PartialEq,
        {
            match self.iter().position(|x| x == valor) {
                Some(pos) => {
                    self.remove(pos);
                    true
                }
                None => false,
            }
        }

        #[inline]
        fn index_of(&self, valor: &T) -> Option<usize>
        where
            T: PartialEq,
        {
            self.iter().position(|x| x == valor)
        }

        #[inline]
        fn count_value(&self, valor: &T) -> usize
        where
            T: PartialEq,
        {
            self.iter().filter(|&x| x == valor).count()
        }
    }
}

// ============================================================================
// 3) Matriz — dados contíguos (cache-friendly em x86-64), não Vec<Vec<T>>
// ============================================================================
pub mod matrix_utils {
    //! Matriz bidimensional densa com armazenamento contíguo (`Vec<T>` linear).
    //! Garante localidade espacial na cache L1/L2 da CPU e habilita auto-vetorização
    //! (SIMD/SSE/AVX) pelo compilador LLVM.

    use super::arithmetic::NumeroPy;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Matrix<T> {
        dados: Vec<T>,
        pub linhas: usize,
        pub colunas: usize,
    }

    impl<T: Clone> Matrix<T> {
        /// Cria uma matriz `linhas x colunas` preenchida com `valor_padrao`.
        /// Equivalente a `[[valor_padrao] * colunas for _ in range(linhas)]`.
        pub fn new(linhas: usize, colunas: usize, valor_padrao: T) -> Self {
            Matrix {
                dados: vec![valor_padrao; linhas.saturating_mul(colunas)],
                linhas,
                colunas,
            }
        }

        /// Cria uma matriz a partir de uma função geradora `f(linha, coluna)`.
        pub fn from_fn<F>(linhas: usize, colunas: usize, mut f: F) -> Self
        where
            F: FnMut(usize, usize) -> T,
        {
            let mut dados = Vec::with_capacity(linhas.saturating_mul(colunas));
            for i in 0..linhas {
                for j in 0..colunas {
                    dados.push(f(i, j));
                }
            }
            Matrix {
                dados,
                linhas,
                colunas,
            }
        }

        /// Acesso seguro com verificação de limites (`Option<&T>`).
        #[inline]
        pub fn get(&self, linha: usize, coluna: usize) -> Option<&T> {
            if linha < self.linhas && coluna < self.colunas {
                Some(&self.dados[linha * self.colunas + coluna])
            } else {
                None
            }
        }

        /// Acesso mutável seguro com verificação de limites (`Option<&mut T>`).
        #[inline]
        pub fn get_mut(&mut self, linha: usize, coluna: usize) -> Option<&mut T> {
            if linha < self.linhas && coluna < self.colunas {
                Some(&mut self.dados[linha * self.colunas + coluna])
            } else {
                None
            }
        }

        /// Devolve uma fatia (`&[T]`) da linha `linha` em $O(1)$, sem alocar.
        #[inline]
        pub fn row(&self, linha: usize) -> Option<&[T]> {
            if linha < self.linhas {
                let ini = linha * self.colunas;
                Some(&self.dados[ini..ini + self.colunas])
            } else {
                None
            }
        }

        /// Devolve a matriz transposta ($M^T$) como uma nova `Matrix<T>`.
        pub fn transpose(&self) -> Matrix<T> {
            if self.dados.is_empty() {
                return Matrix {
                    dados: Vec::new(),
                    linhas: self.colunas,
                    colunas: self.linhas,
                };
            }
            let mut nova = Matrix::new(self.colunas, self.linhas, self.dados[0].clone());
            for i in 0..self.linhas {
                for j in 0..self.colunas {
                    nova[(j, i)] = self[(i, j)].clone();
                }
            }
            nova
        }

        /// Consome a matriz e devolve o `Vec<T>` linear subjacente sem cópia.
        #[inline]
        pub fn flatten(self) -> Vec<T> {
            self.dados
        }
    }

    impl<T: NumeroPy> Matrix<T> {
        /// Multiplicação matricial (`self * other`) otimizada para a cache da CPU.
        ///
        /// Transpõe `other` internamente antes do produto escalar para garantir que
        /// ambos os laços internos percorram a memória de forma estritamente
        /// sequencial (alinhada), facilitando o uso de instruções SIMD pelo LLVM.
        /// Devolve `None` se `self.colunas != other.linhas`.
        pub fn dot(&self, other: &Matrix<T>) -> Option<Matrix<T>> {
            if self.colunas != other.linhas {
                return None;
            }
            let n = self.colunas;
            let other_t = other.transpose();
            let mut res = Matrix::new(self.linhas, other.colunas, T::ZERO);

            for i in 0..self.linhas {
                let linha_a = &self.dados[i * n..(i + 1) * n];
                for j in 0..other.colunas {
                    let linha_bt = &other_t.dados[j * n..(j + 1) * n];
                    let mut soma = T::ZERO;
                    for k in 0..n {
                        soma = soma + linha_a[k] * linha_bt[k];
                    }
                    res[(i, j)] = soma;
                }
            }
            Some(res)
        }
    }

    impl<T> std::ops::Index<(usize, usize)> for Matrix<T> {
        type Output = T;
        #[inline]
        fn index(&self, (linha, coluna): (usize, usize)) -> &T {
            assert!(
                linha < self.linhas && coluna < self.colunas,
                "IndexError: índice ({linha}, {coluna}) fora da matriz {}x{}",
                self.linhas,
                self.colunas
            );
            &self.dados[linha * self.colunas + coluna]
        }
    }

    impl<T> std::ops::IndexMut<(usize, usize)> for Matrix<T> {
        #[inline]
        fn index_mut(&mut self, (linha, coluna): (usize, usize)) -> &mut T {
            assert!(
                linha < self.linhas && coluna < self.colunas,
                "IndexError: índice ({linha}, {coluna}) fora da matriz {}x{}",
                self.linhas,
                self.colunas
            );
            &mut self.dados[linha * self.colunas + coluna]
        }
    }
}

// ============================================================================
// 4) Built-ins gerais: print(), len(), min(), max(), sorted(), reversed(),
//    enumerate(), zip(), any(), all(), histogram(), type_of()
// ============================================================================
pub mod builtins {
    //! Funções livres equivalentes às funções integradas (*built-ins*) do Python.

    use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
    use std::fmt::Display;
    use std::hash::Hash;

    /// `print(x)` — exibe qualquer valor que implemente `Display` seguido de quebra
    /// de linha. Ideal para quem está migrando do Python e quer imprimir valores
    /// simples sem precisar da string de formatação de `println!("{x}")`.
    #[inline]
    pub fn print<T: Display>(valor: T) {
        println!("{valor}");
    }

    /// `len(&x)` — devolve o comprimento da coleção ou string.
    #[inline]
    pub fn len<T: ?Sized + PyLen>(x: &T) -> usize {
        x.py_len()
    }

    pub trait PyLen {
        fn py_len(&self) -> usize;
    }

    impl PyLen for str {
        /// Conta caracteres Unicode (`chars().count()`), igual ao `len()` do Python.
        #[inline]
        fn py_len(&self) -> usize {
            self.chars().count()
        }
    }

    impl PyLen for String {
        #[inline]
        fn py_len(&self) -> usize {
            self.chars().count()
        }
    }

    impl<T> PyLen for [T] {
        #[inline]
        fn py_len(&self) -> usize {
            self.len()
        }
    }

    impl<T> PyLen for Vec<T> {
        #[inline]
        fn py_len(&self) -> usize {
            self.len()
        }
    }

    impl<T> PyLen for VecDeque<T> {
        #[inline]
        fn py_len(&self) -> usize {
            self.len()
        }
    }

    impl<K, V> PyLen for HashMap<K, V> {
        #[inline]
        fn py_len(&self) -> usize {
            self.len()
        }
    }

    impl<T> PyLen for HashSet<T> {
        #[inline]
        fn py_len(&self) -> usize {
            self.len()
        }
    }

    impl<K, V> PyLen for BTreeMap<K, V> {
        #[inline]
        fn py_len(&self) -> usize {
            self.len()
        }
    }

    impl<T> PyLen for BTreeSet<T> {
        #[inline]
        fn py_len(&self) -> usize {
            self.len()
        }
    }

    /// `min(iter)` — devolve `Some(menor)` ou `None` se a sequência for vazia.
    pub fn min<T, I>(iter: I) -> Option<T>
    where
        I: IntoIterator<Item = T>,
        T: PartialOrd,
    {
        iter.into_iter().fold(None, |menor, x| match menor {
            None => Some(x),
            Some(m) => {
                if x < m {
                    Some(x)
                } else {
                    Some(m)
                }
            }
        })
    }

    /// `max(iter)` — devolve `Some(maior)` ou `None` se a sequência for vazia.
    pub fn max<T, I>(iter: I) -> Option<T>
    where
        I: IntoIterator<Item = T>,
        T: PartialOrd,
    {
        iter.into_iter().fold(None, |maior, x| match maior {
            None => Some(x),
            Some(m) => {
                if x > m {
                    Some(x)
                } else {
                    Some(m)
                }
            }
        })
    }

    /// `sorted(iter)` — devolve um novo `Vec<T>` ordenado em ordem crescente.
    pub fn sorted<T, I>(iter: I) -> Vec<T>
    where
        I: IntoIterator<Item = T>,
        T: PartialOrd,
    {
        let mut v: Vec<T> = iter.into_iter().collect();
        v.sort_by(|a, b| {
            a.partial_cmp(b)
                .expect("ValueError: valores incomparáveis em sorted() (ex.: NaN)")
        });
        v
    }

    /// `reversed(iter)` — devolve um iterador reverso preguiçoso (sem alocar).
    #[inline]
    pub fn reversed<I>(iter: I) -> std::iter::Rev<I::IntoIter>
    where
        I: IntoIterator,
        I::IntoIter: DoubleEndedIterator,
    {
        iter.into_iter().rev()
    }

    /// `enumerate(iter)` — produz pares `(indice_usize, elemento)` igual ao Python.
    #[inline]
    pub fn enumerate<I: IntoIterator>(iter: I) -> std::iter::Enumerate<I::IntoIter> {
        iter.into_iter().enumerate()
    }

    /// `zip(a, b)` — caminha por duas sequências em paralelo produzindo tuplas `(a_i, b_i)`.
    #[inline]
    pub fn zip<A: IntoIterator, B: IntoIterator>(
        a: A,
        b: B,
    ) -> std::iter::Zip<A::IntoIter, B::IntoIter> {
        a.into_iter().zip(b)
    }

    /// `any(iter)` — devolve `true` se ao menos um elemento da sequência for `true`.
    #[inline]
    pub fn any<I: IntoIterator<Item = bool>>(iter: I) -> bool {
        iter.into_iter().any(|x| x)
    }

    /// `all(iter)` — devolve `true` se todos os elementos da sequência forem `true`.
    #[inline]
    pub fn all<I: IntoIterator<Item = bool>>(iter: I) -> bool {
        iter.into_iter().all(|x| x)
    }

    /// `histogram(iter)` — equivalente a `collections.Counter(iter)` do Python
    /// (ou à função clássica `histogram` de contagem de frequências em dicionário).
    /// Conta quantas vezes cada item aparece na sequência em tempo $O(N)$.
    pub fn histogram<T, I>(iter: I) -> HashMap<T, usize>
    where
        I: IntoIterator<Item = T>,
        T: Eq + Hash,
    {
        let mut mapa = HashMap::new();
        for item in iter {
            *mapa.entry(item).or_insert(0) += 1;
        }
        mapa
    }

    /// `type_of(&x)` — devolve o nome estático do tipo inferido pelo compilador.
    #[inline]
    pub fn type_of<T>(_: &T) -> &'static str {
        std::any::type_name::<T>()
    }
}

// ============================================================================
// 5) Aritmética — sum(), sub(), mult(), div(), divmod(), pow(), root(), abs()
// ============================================================================
pub mod arithmetic {
    //! Aritmética genérica e protegida contra divisão por zero para todas as
    //! larguras numéricas nativas do Rust (`i8`..`i128`, `isize`, `u8`..`u128`,
    //! `usize`, `f32`, `f64`).

    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub enum ArithError {
        DivisaoPorZero,
        RaizDeNegativoComIndicePar,
        IndiceDeRaizInvalido,
    }

    pub trait NumeroPy:
        Copy
        + PartialEq
        + PartialOrd
        + std::ops::Add<Output = Self>
        + std::ops::Sub<Output = Self>
        + std::ops::Mul<Output = Self>
        + std::ops::Div<Output = Self>
        + std::ops::Rem<Output = Self>
    {
        const ZERO: Self;
        const UM: Self;
        fn abs_val(self) -> Self;
    }

    macro_rules! impl_numero_py_signed {
        ($($tipo:ty => $zero:expr, $um:expr);+ $(;)?) => {
            $(
                impl NumeroPy for $tipo {
                    const ZERO: Self = $zero;
                    const UM: Self = $um;
                    #[inline]
                    fn abs_val(self) -> Self {
                        if self < Self::ZERO { Self::ZERO - self } else { self }
                    }
                }
            )+
        };
    }

    macro_rules! impl_numero_py_unsigned {
        ($($tipo:ty => $zero:expr, $um:expr);+ $(;)?) => {
            $(
                impl NumeroPy for $tipo {
                    const ZERO: Self = $zero;
                    const UM: Self = $um;
                    #[inline]
                    fn abs_val(self) -> Self {
                        self
                    }
                }
            )+
        };
    }

    impl_numero_py_signed!(
        i8 => 0, 1;
        i16 => 0, 1;
        i32 => 0, 1;
        i64 => 0, 1;
        i128 => 0, 1;
        isize => 0, 1;
        f32 => 0.0, 1.0;
        f64 => 0.0, 1.0;
    );

    impl_numero_py_unsigned!(
        u8 => 0, 1;
        u16 => 0, 1;
        u32 => 0, 1;
        u64 => 0, 1;
        u128 => 0, 1;
        usize => 0, 1;
    );

    /// `abs(x)` — valor absoluto genérico para qualquer tipo numérico.
    #[inline]
    pub fn abs<T: NumeroPy>(x: T) -> T {
        x.abs_val()
    }

    /// `sum(iter)` — soma todos os elementos; devolve `0` se vazio (`sum([]) == 0`).
    #[inline]
    pub fn sum<T: NumeroPy, I: IntoIterator<Item = T>>(iter: I) -> T {
        iter.into_iter().fold(T::ZERO, |acc, x| acc + x)
    }

    /// `sub(iter)` — subtração encadeada (`primeiro - segundo - ...`); `None` se vazio.
    pub fn sub<T: NumeroPy, I: IntoIterator<Item = T>>(iter: I) -> Option<T> {
        let mut it = iter.into_iter();
        let primeiro = it.next()?;
        Some(it.fold(primeiro, |acc, x| acc - x))
    }

    /// `mult(iter)` — produtório (`math.prod` do Python); devolve `1` se vazio.
    #[inline]
    pub fn mult<T: NumeroPy, I: IntoIterator<Item = T>>(iter: I) -> T {
        iter.into_iter().fold(T::UM, |acc, x| acc * x)
    }

    /// `div(a, b)` — divisão segura que devolve `Err(ArithError::DivisaoPorZero)` se `b == 0`.
    #[inline]
    pub fn div<T: NumeroPy>(a: T, b: T) -> Result<T, ArithError> {
        if b == T::ZERO {
            Err(ArithError::DivisaoPorZero)
        } else {
            Ok(a / b)
        }
    }

    /// `divmod(a, b)` — equivalente ao `divmod(a, b)` do Python: devolve a tupla
    /// `(quociente, resto)` em uma única operação segura contra divisão por zero.
    #[inline]
    pub fn divmod<T: NumeroPy>(a: T, b: T) -> Result<(T, T), ArithError> {
        if b == T::ZERO {
            Err(ArithError::DivisaoPorZero)
        } else {
            Ok((a / b, a % b))
        }
    }

    /// `pow(base, expoente)` — exponenciação rápida por quadratura em $O(\log(\text{expoente}))$.
    #[inline]
    pub fn pow<T: NumeroPy>(mut base: T, mut expoente: u32) -> T {
        let mut resultado = T::UM;
        while expoente > 0 {
            if expoente & 1 == 1 {
                resultado = resultado * base;
            }
            expoente >>= 1;
            if expoente > 0 {
                base = base * base;
            }
        }
        resultado
    }

    /// `root(x, indice)` — raiz n-ésima real (`f64`).
    /// Retorna erro explícito se `indice` for `0.0`/`NaN`, ou se `x < 0.0` com
    /// índice par ou fracionário.
    pub fn root(x: f64, indice: f64) -> Result<f64, ArithError> {
        if indice == 0.0 || indice.is_nan() {
            return Err(ArithError::IndiceDeRaizInvalido);
        }
        if x < 0.0 {
            if indice.fract() != 0.0 || (indice as i64) % 2 == 0 {
                return Err(ArithError::RaizDeNegativoComIndicePar);
            }
            return Ok(-((-x).powf(1.0 / indice)));
        }
        Ok(x.powf(1.0 / indice))
    }
}

// ============================================================================
// 6) Strings — split(), strip(), join(), replace(), lower(), upper(),
//    startswith(), endswith(), find(), count(), is_palindrome()
// ============================================================================
pub mod string_utils {
    //! Manipulação de strings com foco em zero-copy (`&str`) onde possível e
    //! alocação única nas operações que geram texto novo.

    /// `split(s, sep)` — fatias emprestadas de `s` (zero alocação de caracteres).
    #[inline]
    pub fn split<'a>(s: &'a str, sep: &str) -> Vec<&'a str> {
        s.split(sep).collect()
    }

    /// `strip(s)` — remove espaços em branco do início e do fim sem alocar.
    #[inline]
    pub fn strip(s: &str) -> &str {
        s.trim()
    }

    /// `join(sep, iter)` — equivalente a `sep.join(iter)` do Python.
    /// Faz uma única alocação de `String`, escrevendo direto no buffer final.
    pub fn join<I, S>(sep: &str, iter: I) -> String
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut it = iter.into_iter();
        let primeiro = match it.next() {
            Some(p) => p,
            None => return String::new(),
        };
        let (min_len, _) = it.size_hint();
        let mut resultado =
            String::with_capacity(primeiro.as_ref().len() + min_len * (sep.len() + 4));
        resultado.push_str(primeiro.as_ref());
        for item in it {
            resultado.push_str(sep);
            resultado.push_str(item.as_ref());
        }
        resultado
    }

    /// `replace(s, de, para)` — equivalente a `s.replace(de, para)` do Python.
    #[inline]
    pub fn replace(s: &str, de: &str, para: &str) -> String {
        s.replace(de, para)
    }

    /// `lower(s)` — equivalente a `s.lower()` do Python.
    #[inline]
    pub fn lower(s: &str) -> String {
        s.to_lowercase()
    }

    /// `upper(s)` — equivalente a `s.upper()` do Python.
    #[inline]
    pub fn upper(s: &str) -> String {
        s.to_uppercase()
    }

    /// `startswith(s, prefixo)` — equivalente a `s.startswith(prefixo)`.
    #[inline]
    pub fn startswith(s: &str, prefixo: &str) -> bool {
        s.starts_with(prefixo)
    }

    /// `endswith(s, sufixo)` — equivalente a `s.endswith(sufixo)`.
    #[inline]
    pub fn endswith(s: &str, sufixo: &str) -> bool {
        s.ends_with(sufixo)
    }

    /// `find(s, sub)` — devolve o índice **em caracteres Unicode** (igual ao
    /// `s.find(sub)` do Python) ou `-1` se não encontrar.
    pub fn find(s: &str, sub: &str) -> i64 {
        match s.find(sub) {
            Some(byte_idx) => s[..byte_idx].chars().count() as i64,
            None => -1,
        }
    }

    /// `count(s, sub)` — conta ocorrências não-sobrepostas de `sub` em `s`
    /// (`s.count(sub)` do Python).
    #[inline]
    pub fn count(s: &str, sub: &str) -> usize {
        s.matches(sub).count()
    }

    /// `is_palindrome(s)` — verifica em $O(N)$ e zero alocação se `s` é um
    /// palíndromo (lê-se igual de trás para frente em caracteres Unicode).
    #[inline]
    pub fn is_palindrome(s: &str) -> bool {
        let mut chars = s.chars();
        while let (Some(a), Some(b)) = (chars.next(), chars.next_back()) {
            if a != b {
                return false;
            }
        }
        true
    }
}

// ============================================================================
// 7) OOP estilo C++/Python — macro `class!` (com suporte a herança `:`)
// ============================================================================

/// Declara uma estrutura e seus métodos em um único bloco estilo C++/Python,
/// gerando construtor `new(...)` automático e suportando herança (`struct Filha : Pai`)
/// via composição + `Deref`/`DerefMut` (custo zero).
#[macro_export]
macro_rules! class {
    // 1) Classe base padrão
    (
        struct $nome:ident {
            $( $campo:ident : $tipo:ty ),* $(,)?
        }
        impl {
            $( fn $metodo:ident ( $($params:tt)* ) $(-> $ret:ty)? $corpo:block )*
        }
    ) => {
        #[derive(Debug, Clone, PartialEq)]
        pub struct $nome {
            $( pub $campo: $tipo ),*
        }

        impl $nome {
            /// Construtor gerado automaticamente a partir dos campos declarados.
            pub fn new( $( $campo: $tipo ),* ) -> Self {
                Self { $( $campo ),* }
            }

            $(
                pub fn $metodo( $($params)* ) $(-> $ret)? $corpo
            )*
        }
    };

    // 2) Classe derivada com herança estilo C++ (`struct Filha : Pai { ... }`)
    (
        struct $nome:ident : $pai:ty {
            $( $campo:ident : $tipo:ty ),* $(,)?
        }
        impl {
            $( fn $metodo:ident ( $($params:tt)* ) $(-> $ret:ty)? $corpo:block )*
        }
    ) => {
        #[derive(Debug, Clone, PartialEq)]
        pub struct $nome {
            pub base: $pai,
            $( pub $campo: $tipo ),*
        }

        impl $nome {
            /// Construtor que recebe a instância da classe `base` seguida dos campos próprios.
            pub fn new(base: $pai, $( $campo: $tipo ),* ) -> Self {
                Self { base, $( $campo ),* }
            }

            $(
                pub fn $metodo( $($params)* ) $(-> $ret)? $corpo
            )*
        }

        impl ::std::ops::Deref for $nome {
            type Target = $pai;
            #[inline]
            fn deref(&self) -> &Self::Target {
                &self.base
            }
        }

        impl ::std::ops::DerefMut for $nome {
            #[inline]
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.base
            }
        }
    };
}

// ============================================================================
// 8) Tempo — time(), sleep(), perf_counter(), timeit(), gmtime()/strftime()
// ============================================================================
pub mod time_utils {
    //! Equivalente (100% `std`) aos módulos `time`, `timeit` e `calendar` do Python.

    use std::fmt::Write as _;
    use std::hint::black_box;
    use std::sync::OnceLock;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    /// `time.time()` — segundos desde 1970-01-01 00:00:00 UTC como `f64`.
    #[inline]
    pub fn time() -> f64 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(d) => d.as_secs_f64(),
            Err(e) => -e.duration().as_secs_f64(),
        }
    }

    /// `time.sleep(secs)` — suspende a thread atual por `secs` segundos.
    ///
    /// # Panics
    /// Causa panic se `secs` for negativo, `NaN` ou infinito (`ValueError` no Python).
    pub fn sleep(secs: f64) {
        let d = Duration::try_from_secs_f64(secs)
            .unwrap_or_else(|_| panic!("ValueError: sleep length inválido ({secs})"));
        std::thread::sleep(d);
    }

    fn origem() -> Instant {
        static ORIGEM: OnceLock<Instant> = OnceLock::new();
        *ORIGEM.get_or_init(Instant::now)
    }

    /// `time.perf_counter()` — relógio monotônico de alta precisão em segundos (`f64`).
    #[inline]
    pub fn perf_counter() -> f64 {
        origem().elapsed().as_secs_f64()
    }

    /// `time.perf_counter_ns()` — relógio monotônico em nanossegundos (`u64`).
    #[inline]
    pub fn perf_counter_ns() -> u64 {
        origem().elapsed().as_nanos() as u64
    }

    /// `timeit.timeit(f, number=n)` — executa `f` `number` vezes protegendo o
    /// resultado com `black_box` e devolve o tempo total em segundos.
    pub fn timeit<R, F: FnMut() -> R>(number: u32, mut f: F) -> f64 {
        let inicio = Instant::now();
        for _ in 0..number {
            black_box(f());
        }
        inicio.elapsed().as_secs_f64()
    }

    /// `timeit.repeat(f, repeat=r, number=n)` — executa `r` baterias de `n` execuções.
    pub fn timeit_repeat<R, F: FnMut() -> R>(repeat: u32, number: u32, mut f: F) -> Vec<f64> {
        (0..repeat).map(|_| timeit(number, &mut f)).collect()
    }

    /// Macro ergonômica de benchmark: `timeit!(1_000, { codigo() })` ou `timeit!({ codigo() })`.
    #[macro_export]
    macro_rules! timeit {
        ($body:block) => {
            $crate::time_utils::timeit(1, || $body)
        };
        ($n:expr, $body:block) => {
            $crate::time_utils::timeit($n, || $body)
        };
    }

    /// Equivalente a `time.struct_time` em UTC (`weekday`: 0 = segunda .. 6 = domingo).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StructTime {
        pub year: i64,
        pub month: u32,
        pub day: u32,
        pub hour: u32,
        pub minute: u32,
        pub second: u32,
        pub weekday: u32,
        pub yearday: u32,
    }

    fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = if m > 2 { m as i64 - 3 } else { m as i64 + 9 };
        let doy = (153 * mp + 2) / 5 + d as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    fn civil_from_days(z: i64) -> (i64, u32, u32) {
        let z = z + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
        (y, m, d)
    }

    /// `time.gmtime(secs)` — converte timestamp epoch em `StructTime` UTC.
    pub fn gmtime(secs: i64) -> StructTime {
        let dias = secs.div_euclid(86_400);
        let resto = secs.rem_euclid(86_400) as u32;
        let (year, month, day) = civil_from_days(dias);
        StructTime {
            year,
            month,
            day,
            hour: resto / 3_600,
            minute: resto % 3_600 / 60,
            second: resto % 60,
            weekday: (dias + 3).rem_euclid(7) as u32,
            yearday: (dias - days_from_civil(year, 1, 1) + 1) as u32,
        }
    }

    /// `gmtime_now()` — devolve a data/hora UTC atual.
    #[inline]
    pub fn gmtime_now() -> StructTime {
        gmtime(time().floor() as i64)
    }

    /// `calendar.timegm(t)` — converte `StructTime` UTC de volta para segundos epoch.
    #[inline]
    pub fn mktime_utc(t: &StructTime) -> i64 {
        days_from_civil(t.year, t.month, t.day) * 86_400
            + t.hour as i64 * 3_600
            + t.minute as i64 * 60
            + t.second as i64
    }

    const DIAS: [&str; 7] = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ];
    const MESES: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];

    /// `time.strftime(fmt, t)` — formata `StructTime` segundo as diretivas padrão C/Python.
    pub fn strftime(fmt: &str, t: &StructTime) -> String {
        let mut out = String::with_capacity(fmt.len() + 16);
        let mut chars = fmt.chars();
        while let Some(c) = chars.next() {
            if c != '%' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('Y') => {
                    let _ = write!(out, "{}", t.year);
                }
                Some('y') => {
                    let _ = write!(out, "{:02}", t.year.rem_euclid(100));
                }
                Some('m') => {
                    let _ = write!(out, "{:02}", t.month);
                }
                Some('d') => {
                    let _ = write!(out, "{:02}", t.day);
                }
                Some('H') => {
                    let _ = write!(out, "{:02}", t.hour);
                }
                Some('M') => {
                    let _ = write!(out, "{:02}", t.minute);
                }
                Some('S') => {
                    let _ = write!(out, "{:02}", t.second);
                }
                Some('I') => {
                    let h = if t.hour % 12 == 0 { 12 } else { t.hour % 12 };
                    let _ = write!(out, "{:02}", h);
                }
                Some('p') => out.push_str(if t.hour < 12 { "AM" } else { "PM" }),
                Some('j') => {
                    let _ = write!(out, "{:03}", t.yearday);
                }
                Some('a') => out.push_str(&DIAS[t.weekday as usize % 7][..3]),
                Some('A') => out.push_str(DIAS[t.weekday as usize % 7]),
                Some('b') => out.push_str(&MESES[(t.month as usize + 11) % 12][..3]),
                Some('B') => out.push_str(MESES[(t.month as usize + 11) % 12]),
                Some('Z') => out.push_str("UTC"),
                Some('F') => {
                    let _ = write!(out, "{}-{:02}-{:02}", t.year, t.month, t.day);
                }
                Some('T') => {
                    let _ = write!(out, "{:02}:{:02}:{:02}", t.hour, t.minute, t.second);
                }
                Some('%') => out.push('%'),
                Some(outro) => {
                    out.push('%');
                    out.push(outro);
                }
                None => out.push('%'),
            }
        }
        out
    }
}

// ============================================================================
// 9) Rede — módulo `socket` do Python sobre std::net (TCP, UDP, DNS)
// ============================================================================
pub mod net_utils {
    //! Wrappers de custo zero sobre `std::net` com ergonomia de `socket` do Python.

    use std::io::{self, Read, Write};
    use std::net::{self, Ipv4Addr, Shutdown, SocketAddr, ToSocketAddrs};
    use std::time::Duration;

    fn para_duracao(secs: Option<f64>) -> io::Result<Option<Duration>> {
        match secs {
            None => Ok(None),
            Some(s) => Duration::try_from_secs_f64(s).map(Some).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "timeout inválido (negativo/NaN/infinito)",
                )
            }),
        }
    }

    #[derive(Debug)]
    pub struct TcpClient {
        stream: net::TcpStream,
    }

    impl TcpClient {
        pub fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
            Ok(TcpClient {
                stream: net::TcpStream::connect(addr)?,
            })
        }

        pub fn connect_timeout<A: ToSocketAddrs>(addr: A, secs: f64) -> io::Result<Self> {
            let dur = para_duracao(Some(secs))?.unwrap_or_default();
            let mut ultimo = io::Error::new(io::ErrorKind::NotFound, "nenhum endereço resolvido");
            for a in addr.to_socket_addrs()? {
                match net::TcpStream::connect_timeout(&a, dur) {
                    Ok(stream) => return Ok(TcpClient { stream }),
                    Err(e) => ultimo = e,
                }
            }
            Err(ultimo)
        }

        pub fn send<D: AsRef<[u8]>>(&mut self, dados: D) -> io::Result<()> {
            self.stream.write_all(dados.as_ref())
        }

        pub fn recv(&mut self, max: usize) -> io::Result<Vec<u8>> {
            let mut buf = vec![0u8; max];
            let n = self.stream.read(&mut buf)?;
            buf.truncate(n);
            Ok(buf)
        }

        pub fn recv_into(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.stream.read(buf)
        }

        pub fn recv_exact(&mut self, n: usize) -> io::Result<Vec<u8>> {
            let mut buf = vec![0u8; n];
            self.stream.read_exact(&mut buf)?;
            Ok(buf)
        }

        pub fn recv_string(&mut self, max: usize) -> io::Result<String> {
            Ok(String::from_utf8_lossy(&self.recv(max)?).into_owned())
        }

        pub fn set_timeout(&mut self, secs: Option<f64>) -> io::Result<()> {
            let d = para_duracao(secs)?;
            self.stream.set_read_timeout(d)?;
            self.stream.set_write_timeout(d)
        }

        pub fn set_nodelay(&mut self, ligado: bool) -> io::Result<()> {
            self.stream.set_nodelay(ligado)
        }

        pub fn peer_addr(&self) -> io::Result<SocketAddr> {
            self.stream.peer_addr()
        }

        pub fn local_addr(&self) -> io::Result<SocketAddr> {
            self.stream.local_addr()
        }

        pub fn close(self) -> io::Result<()> {
            self.stream.shutdown(Shutdown::Both)
        }
    }

    #[derive(Debug)]
    pub struct TcpServer {
        listener: net::TcpListener,
    }

    impl TcpServer {
        pub fn listen<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
            Ok(TcpServer {
                listener: net::TcpListener::bind(addr)?,
            })
        }

        pub fn accept(&self) -> io::Result<(TcpClient, SocketAddr)> {
            let (stream, addr) = self.listener.accept()?;
            Ok((TcpClient { stream }, addr))
        }

        pub fn incoming(&self) -> impl Iterator<Item = io::Result<TcpClient>> + '_ {
            self.listener
                .incoming()
                .map(|r| r.map(|stream| TcpClient { stream }))
        }

        pub fn local_addr(&self) -> io::Result<SocketAddr> {
            self.listener.local_addr()
        }
    }

    #[derive(Debug)]
    pub struct UdpSock {
        socket: net::UdpSocket,
    }

    impl UdpSock {
        pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
            Ok(UdpSock {
                socket: net::UdpSocket::bind(addr)?,
            })
        }

        pub fn send_to<D: AsRef<[u8]>, A: ToSocketAddrs>(
            &self,
            dados: D,
            addr: A,
        ) -> io::Result<usize> {
            self.socket.send_to(dados.as_ref(), addr)
        }

        pub fn recv_from(&self, max: usize) -> io::Result<(Vec<u8>, SocketAddr)> {
            let mut buf = vec![0u8; max];
            let (n, origem) = self.socket.recv_from(&mut buf)?;
            buf.truncate(n);
            Ok((buf, origem))
        }

        pub fn recv_from_into(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
            self.socket.recv_from(buf)
        }

        pub fn set_timeout(&self, secs: Option<f64>) -> io::Result<()> {
            let d = para_duracao(secs)?;
            self.socket.set_read_timeout(d)?;
            self.socket.set_write_timeout(d)
        }

        pub fn local_addr(&self) -> io::Result<SocketAddr> {
            self.socket.local_addr()
        }
    }

    pub fn getaddrinfo(host: &str, port: u16) -> io::Result<Vec<SocketAddr>> {
        Ok((host, port).to_socket_addrs()?.collect())
    }

    pub fn gethostbyname(host: &str) -> io::Result<Ipv4Addr> {
        (host, 0)
            .to_socket_addrs()?
            .find_map(|a| match a {
                SocketAddr::V4(v4) => Some(*v4.ip()),
                SocketAddr::V6(_) => None,
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("nenhum IPv4 para '{host}'"),
                )
            })
    }
}

// ============================================================================
// 10) Sistema de arquivos — os / os.path / pathlib / shutil + I/O de texto
// ============================================================================
pub mod fs_utils {
    //! Operações de sistema de arquivos e leitura/escrita rápida de arquivos
    //! equivalentes a `os`, `os.path`, `shutil` e `open()` do Python.

    use std::fs::{self, OpenOptions};
    use std::io::{self, Write};
    #[cfg(unix)]
    use std::os::unix::fs::{symlink as unix_symlink, MetadataExt, PermissionsExt};
    use std::path::{Component, Path, PathBuf};

    // ---------------- Leitura e escrita rápida (open().read() / write()) ----

    /// Lê todo o conteúdo de um arquivo de texto para uma `String`
    /// (equivalente a `open(caminho).read()` no Python).
    #[inline]
    pub fn read_text<P: AsRef<Path>>(caminho: P) -> io::Result<String> {
        fs::read_to_string(caminho)
    }

    /// Lê todas as linhas de um arquivo de texto sem os terminadores `\n`/`\r\n`
    /// (equivalente a `[line.rstrip('\r\n') for line in open(caminho)]`).
    pub fn readlines<P: AsRef<Path>>(caminho: P) -> io::Result<Vec<String>> {
        Ok(fs::read_to_string(caminho)?
            .lines()
            .map(String::from)
            .collect())
    }

    /// Escreve (sobrescreve ou cria) texto em um arquivo
    /// (equivalente a `open(caminho, 'w').write(conteudo)`).
    #[inline]
    pub fn write_text<P: AsRef<Path>, C: AsRef<[u8]>>(caminho: P, conteudo: C) -> io::Result<()> {
        fs::write(caminho, conteudo)
    }

    /// Acrescenta texto ao final de um arquivo, criando-o se não existir
    /// (equivalente a `open(caminho, 'a').write(conteudo)`).
    pub fn append_text<P: AsRef<Path>, C: AsRef<[u8]>>(caminho: P, conteudo: C) -> io::Result<()> {
        let mut arq = OpenOptions::new().create(true).append(true).open(caminho)?;
        arq.write_all(conteudo.as_ref())
    }

    // ---------------- Consultas (os.path.exists/isfile/isdir/islink) -------
    #[inline]
    pub fn exists<P: AsRef<Path>>(caminho: P) -> bool {
        caminho.as_ref().exists()
    }
    #[inline]
    pub fn isfile<P: AsRef<Path>>(caminho: P) -> bool {
        caminho.as_ref().is_file()
    }
    #[inline]
    pub fn isdir<P: AsRef<Path>>(caminho: P) -> bool {
        caminho.as_ref().is_dir()
    }
    #[inline]
    pub fn islink<P: AsRef<Path>>(caminho: P) -> bool {
        caminho.as_ref().is_symlink()
    }

    // ---------------- Listar / criar / apagar (os / shutil) ----------------
    pub fn listdir<P: AsRef<Path>>(caminho: P) -> io::Result<Vec<String>> {
        fs::read_dir(caminho)?
            .map(|entrada| Ok(entrada?.file_name().to_string_lossy().into_owned()))
            .collect()
    }

    pub fn mkdir<P: AsRef<Path>>(caminho: P) -> io::Result<()> {
        fs::create_dir(caminho)
    }

    pub fn makedirs<P: AsRef<Path>>(caminho: P) -> io::Result<()> {
        fs::create_dir_all(caminho)
    }

    pub fn remove<P: AsRef<Path>>(caminho: P) -> io::Result<()> {
        fs::remove_file(caminho)
    }

    pub fn rmdir<P: AsRef<Path>>(caminho: P) -> io::Result<()> {
        fs::remove_dir(caminho)
    }

    pub fn rmtree<P: AsRef<Path>>(caminho: P) -> io::Result<()> {
        fs::remove_dir_all(caminho)
    }

    pub fn rename<P: AsRef<Path>, Q: AsRef<Path>>(de: P, para: Q) -> io::Result<()> {
        fs::rename(de, para)
    }

    pub fn copy<P: AsRef<Path>, Q: AsRef<Path>>(de: P, para: Q) -> io::Result<u64> {
        fs::copy(de, para)
    }

    // ---------------- Diretório atual (os.getcwd/chdir) --------------------
    pub fn getcwd() -> io::Result<String> {
        Ok(std::env::current_dir()?.to_string_lossy().into_owned())
    }

    pub fn chdir<P: AsRef<Path>>(caminho: P) -> io::Result<()> {
        std::env::set_current_dir(caminho)
    }

    // ---------------- Manipulação de caminhos (os.path) --------------------
    pub fn join_path(partes: &[&str]) -> String {
        let mut base = PathBuf::new();
        for p in partes {
            base.push(p);
        }
        base.to_string_lossy().into_owned()
    }

    pub fn basename<P: AsRef<Path>>(caminho: P) -> String {
        caminho
            .as_ref()
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    pub fn dirname<P: AsRef<Path>>(caminho: P) -> String {
        caminho
            .as_ref()
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    pub fn splitext<P: AsRef<Path>>(caminho: P) -> (String, String) {
        let p = caminho.as_ref();
        let stem = p
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let ext = p
            .extension()
            .map(|s| format!(".{}", s.to_string_lossy()))
            .unwrap_or_default();
        let raiz = match p.parent().filter(|d| !d.as_os_str().is_empty()) {
            Some(dir) => dir.join(&stem).to_string_lossy().into_owned(),
            None => stem,
        };
        (raiz, ext)
    }

    pub fn abspath<P: AsRef<Path>>(caminho: P) -> io::Result<String> {
        let caminho = caminho.as_ref();
        let base = if caminho.is_absolute() {
            caminho.to_path_buf()
        } else {
            std::env::current_dir()?.join(caminho)
        };
        let mut normalizado = PathBuf::new();
        for componente in base.components() {
            match componente {
                Component::ParentDir => {
                    normalizado.pop();
                }
                Component::CurDir => {}
                outro => normalizado.push(outro.as_os_str()),
            }
        }
        Ok(normalizado.to_string_lossy().into_owned())
    }

    pub fn realpath<P: AsRef<Path>>(caminho: P) -> io::Result<String> {
        Ok(fs::canonicalize(caminho)?.to_string_lossy().into_owned())
    }

    pub fn expanduser(caminho: &str) -> String {
        if let Some(resto) = caminho.strip_prefix('~') {
            if resto.is_empty() || resto.starts_with('/') {
                if let Ok(home) = std::env::var("HOME") {
                    return format!("{home}{resto}");
                }
            }
        }
        caminho.to_string()
    }

    // ---------------- Percurso recursivo seguro (os.walk) ------------------
    pub type WalkItem = (String, Vec<String>, Vec<String>);

    pub struct Walk {
        pilha: Vec<PathBuf>,
    }

    pub fn walk<P: AsRef<Path>>(raiz: P) -> Walk {
        Walk {
            pilha: vec![raiz.as_ref().to_path_buf()],
        }
    }

    impl Iterator for Walk {
        type Item = io::Result<WalkItem>;

        fn next(&mut self) -> Option<Self::Item> {
            let dir = self.pilha.pop()?;
            let entradas = match fs::read_dir(&dir) {
                Ok(e) => e,
                Err(erro) => return Some(Err(erro)),
            };

            let mut subdirs = Vec::new();
            let mut arquivos = Vec::new();
            for entrada in entradas {
                let entrada = match entrada {
                    Ok(e) => e,
                    Err(erro) => return Some(Err(erro)),
                };
                let nome = entrada.file_name().to_string_lossy().into_owned();
                // Usa file_type() em vez de path().is_dir(): não segue links
                // simbólicos (impede loop infinito) e evita syscall stat() extra!
                let eh_dir = entrada.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                if eh_dir {
                    self.pilha.push(entrada.path());
                    subdirs.push(nome);
                } else {
                    arquivos.push(nome);
                }
            }

            Some(Ok((
                dir.to_string_lossy().into_owned(),
                subdirs,
                arquivos,
            )))
        }
    }

    // ---------------- Glob mínimo (* e ?) ----------------------------------
    fn glob_casa(padrao: &[char], nome: &[char]) -> bool {
        match (padrao.first(), nome.first()) {
            (None, None) => true,
            (Some('*'), _) => {
                glob_casa(&padrao[1..], nome)
                    || (!nome.is_empty() && glob_casa(padrao, &nome[1..]))
            }
            (Some('?'), Some(_)) => glob_casa(&padrao[1..], &nome[1..]),
            (Some(p), Some(n)) if p == n => glob_casa(&padrao[1..], &nome[1..]),
            _ => false,
        }
    }

    pub fn glob(padrao: &str) -> io::Result<Vec<String>> {
        let caminho_padrao = Path::new(padrao);
        let dir = caminho_padrao.parent().filter(|p| !p.as_os_str().is_empty());
        let nome_padrao: Vec<char> = caminho_padrao
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
            .chars()
            .collect();

        let dir_busca = dir
            .map(|d| d.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let mut resultado = Vec::new();
        for entrada in fs::read_dir(&dir_busca)? {
            let entrada = entrada?;
            let nome = entrada.file_name().to_string_lossy().into_owned();
            let nome_chars: Vec<char> = nome.chars().collect();
            if glob_casa(&nome_padrao, &nome_chars) {
                resultado.push(match dir {
                    Some(d) => d.join(&nome).to_string_lossy().into_owned(),
                    None => nome,
                });
            }
        }
        Ok(resultado)
    }

    // ---------------- Stat / permissões / links (Linux/Unix) ---------------
    #[cfg(unix)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Stat {
        pub size: u64,
        pub mtime: i64,
        pub mode: u32,
    }

    #[cfg(unix)]
    pub fn stat<P: AsRef<Path>>(caminho: P) -> io::Result<Stat> {
        let meta = fs::metadata(caminho)?;
        Ok(Stat {
            size: meta.size(),
            mtime: meta.mtime(),
            mode: meta.mode() & 0o7777,
        })
    }

    #[cfg(unix)]
    pub fn chmod<P: AsRef<Path>>(caminho: P, modo: u32) -> io::Result<()> {
        let mut perms = fs::metadata(&caminho)?.permissions();
        perms.set_mode(modo);
        fs::set_permissions(caminho, perms)
    }

    #[cfg(unix)]
    pub fn symlink<P: AsRef<Path>, Q: AsRef<Path>>(alvo: P, link: Q) -> io::Result<()> {
        unix_symlink(alvo, link)
    }

    pub fn readlink<P: AsRef<Path>>(link: P) -> io::Result<String> {
        Ok(fs::read_link(link)?.to_string_lossy().into_owned())
    }
}

// ============================================================================
// 11) Números Aleatórios — módulo `random` do Python em std pura (v1.4.0)
// ============================================================================
pub mod random_utils {
    //! Equivalente ao módulo `random` do Python sem depender de crates externas.
    //! Usa um gerador **SplitMix64 / Xorshift64\*** rápido e thread-safe,
    //! inicializado automaticamente com entropia do relógio de alta resolução
    //! e ID do processo/thread, ou manualmente via [`seed`] para reprodutibilidade.
    //!
    //! | Python                      | Aqui                         |
    //! |-----------------------------|------------------------------|
    //! | `random.seed(s)`            | `seed(s)`                    |
    //! | `random.random()`           | `random()` → `f64` em `[0,1)`|
    //! | `random.randint(a, b)`      | `randint(a, b)` (inclusivo!) |
    //! | `random.choice(seq)`        | `choice(&seq)`               |
    //! | `random.shuffle(seq)`       | `shuffle(&mut seq)`          |

    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static ESTADO_RNG: AtomicU64 = AtomicU64::new(0);

    fn semente_automatica() -> u64 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        let pid = std::process::id() as u64;
        let end_pilha = &nanos as *const u64 as usize as u64;
        let mut z = nanos ^ pid.rotate_left(17) ^ end_pilha.rotate_right(11);
        if z == 0 {
            z = 0xCAFE_BABE_DEAD_BEEF;
        }
        z
    }

    /// Avança o estado interno usando o passo de mistura SplitMix64 e devolve `u64`.
    fn next_u64() -> u64 {
        let mut atual = ESTADO_RNG.load(Ordering::Relaxed);
        if atual == 0 {
            let inicial = semente_automatica();
            let _ = ESTADO_RNG.compare_exchange(
                0,
                inicial,
                Ordering::Relaxed,
                Ordering::Relaxed,
            );
            atual = ESTADO_RNG.load(Ordering::Relaxed);
        }
        loop {
            let proximo = atual.wrapping_add(0x9E37_79B9_7F4A_7C15);
            if ESTADO_RNG
                .compare_exchange_weak(atual, proximo, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                let mut z = proximo;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                return z ^ (z >> 31);
            }
            atual = ESTADO_RNG.load(Ordering::Relaxed);
        }
    }

    /// `random.seed(valor)` — define uma semente fixa para resultados determinísticos.
    #[inline]
    pub fn seed(valor: u64) {
        let s = if valor == 0 { 1 } else { valor };
        ESTADO_RNG.store(s, Ordering::Relaxed);
    }

    /// `random.random()` — devolve um `f64` uniforme no intervalo semiaberto `[0.0, 1.0)`.
    #[inline]
    pub fn random() -> f64 {
        // Usa os 53 bits mais significativos (precisão exata da mantissa IEEE-754 f64)
        ((next_u64() >> 11) as f64) * (1.0 / ((1u64 << 53) as f64))
    }

    /// `random.randint(low, high)` — devolve um inteiro aleatório `N` tal que
    /// `low <= N <= high` (ambos inclusivos, igual ao `randint` do Python!).
    ///
    /// # Panics
    /// Causa panic se `low > high` (`ValueError` no Python).
    pub fn randint(low: i64, high: i64) -> i64 {
        assert!(
            low <= high,
            "ValueError: intervalo vazio em randint({low}, {high})"
        );
        let largura = (high as i128 - low as i128 + 1) as u128;
        let sorteado = (next_u64() as u128) % largura;
        (low as i128 + sorteado as i128) as i64
    }

    /// `random.choice(seq)` — escolhe um elemento aleatório de uma fatia (`None` se vazia).
    #[inline]
    pub fn choice<T>(fatia: &[T]) -> Option<&T> {
        if fatia.is_empty() {
            None
        } else {
            let idx = (next_u64() as usize) % fatia.len();
            Some(&fatia[idx])
        }
    }

    /// `random.shuffle(seq)` — embaralha uma fatia ou `Vec<T>` in-place usando
    /// o algoritmo de Fisher-Yates em $O(N)$.
    pub fn shuffle<T>(fatia: &mut [T]) {
        let n = fatia.len();
        for i in (1..n).rev() {
            let j = (next_u64() as usize) % (i + 1);
            fatia.swap(i, j);
        }
    }
}

// ============================================================================
// Prelude — import único e ergonômico: `use pyrsutils::prelude::*;`
// ============================================================================
pub mod prelude {
    pub use super::arithmetic::{abs, div, divmod, mult, pow, root, sub, sum, ArithError, NumeroPy};
    pub use super::builtins::{
        all, any, enumerate, histogram, len, max, min, print, reversed, sorted, type_of, zip,
        PyLen,
    };
    pub use super::list_utils::PyList;
    pub use super::matrix_utils::Matrix;
    pub use super::range_utils::{range, range_from, range_step, PyRange};
    pub use super::string_utils::{
        count, endswith, find, is_palindrome, join, lower, replace, split, startswith, strip,
        upper,
    };
    pub use super::{input, try_input, InputError};
    // v1.1 — tempo e rede
    pub use super::net_utils::{getaddrinfo, gethostbyname, TcpClient, TcpServer, UdpSock};
    pub use super::time_utils::{
        gmtime, gmtime_now, mktime_utc, perf_counter, perf_counter_ns, sleep, strftime, time,
        timeit, timeit_repeat, StructTime,
    };
    // v1.2 / v1.4 — sistema de arquivos + I/O rápido de texto
    pub use super::fs_utils::{
        abspath, append_text, basename, chdir, copy, dirname, exists, expanduser, getcwd, glob,
        isdir, isfile, islink, join_path, listdir, makedirs, mkdir, read_text, readlines,
        readlink, realpath, remove, rename, rmdir, rmtree, splitext, walk, write_text, Walk,
        WalkItem,
    };
    #[cfg(unix)]
    pub use super::fs_utils::{chmod, stat, symlink, Stat};
    // v1.4 — números aleatórios
    pub use super::random_utils::{choice, randint, random, seed, shuffle};
}

// ============================================================================
// Suíte Completa de Testes (v1.0 + v1.1 + v1.2 + Novos Testes v1.4.0)
// ============================================================================
#[cfg(test)]
mod tests {
    #[test]
    fn string_sempre_converte() {
        let texto = "ola mundo".to_string();
        let r: Result<String, _> = texto.parse();
        assert_eq!(r.unwrap(), "ola mundo");
    }

    #[test]
    fn inteiro_converte() {
        let r: Result<i32, _> = "42".parse();
        assert_eq!(r.unwrap(), 42);
    }

    #[test]
    fn float_converte() {
        let r: Result<f64, _> = "3.14".parse();
        assert_eq!(r.unwrap(), 3.14);
    }

    #[test]
    fn bool_converte() {
        let r: Result<bool, _> = "true".parse();
        assert_eq!(r.unwrap(), true);
    }

    #[test]
    fn inteiro_invalido_falha() {
        let r: Result<i32, _> = "abc".parse();
        assert!(r.is_err());
    }
}

#[cfg(test)]
mod tests_extensoes {
    use super::*;

    #[test]
    fn range_basico_e_contains_funcionam() {
        let r = range_utils::range(5);
        assert!(r.contains(3));
        assert!(!r.contains(5));
        let v: Vec<i64> = r.collect();
        assert_eq!(v, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn range_step_negativo_funciona() {
        let r = range_utils::range_step(10, 0, -2);
        assert!(r.contains(8));
        assert!(!r.contains(7));
        let v: Vec<i64> = r.collect();
        assert_eq!(v, vec![10, 8, 6, 4, 2]);
    }

    #[test]
    fn range_tem_len_sem_iterar() {
        use builtins::len;
        assert_eq!(len(&range_utils::range(10)), 10);
        assert_eq!(len(&range_utils::range_step(10, 0, -2)), 5);
        assert_eq!(len(&range_utils::range_from(5, 5)), 0);
        assert_eq!(len(&range_utils::range_from(5, 2)), 0);
    }

    #[test]
    fn lista_append_pop_remove_extend_index() {
        use list_utils::PyList;
        let mut v = vec![1, 2, 3];
        v.append_item(4);
        assert_eq!(v, vec![1, 2, 3, 4]);
        assert_eq!(v.pop_at(None), Some(4));
        assert_eq!(v.pop_at(Some(0)), Some(1));
        assert_eq!(v, vec![2, 3]);
        assert!(v.remove_value(&3));
        assert_eq!(v, vec![2]);
        v.extend_from(vec![5, 6, 2]);
        assert_eq!(v.index_of(&5), Some(1));
        assert_eq!(v.count_value(&2), 2);
    }

    #[test]
    fn matriz_get_set_transpose_e_dot() {
        use matrix_utils::Matrix;
        let mut m = Matrix::new(2, 3, 0);
        m[(0, 0)] = 1;
        m[(1, 2)] = 9;
        assert_eq!(m.get(0, 0), Some(&1));
        assert_eq!(m.get(1, 2), Some(&9));
        let t = m.transpose();
        assert_eq!(t[(0, 0)], 1);
        assert_eq!(t[(2, 1)], 9);

        // Multiplicação matricial (dot): (2x3) * (3x2) = (2x2)
        let prod = m.dot(&t).unwrap();
        assert_eq!((prod.linhas, prod.colunas), (2, 2));
        assert_eq!(prod[(0, 0)], 1);
        assert_eq!(prod[(1, 1)], 81);
    }

    #[test]
    fn builtins_completos() {
        use builtins::*;
        assert_eq!(min(vec![3, 1, 2]), Some(1));
        assert_eq!(max(vec![3, 1, 2]), Some(3));
        assert_eq!(sorted(vec![3, 1, 2]), vec![1, 2, 3]);
        let r: Vec<i32> = reversed(vec![1, 2, 3]).collect();
        assert_eq!(r, vec![3, 2, 1]);
        assert_eq!(len("olá"), 3);
        assert_eq!(len(&String::from("café")), 4);
        assert_eq!(len(&vec![10, 20]), 2);

        let pares: Vec<(usize, &str)> = enumerate(vec!["a", "b"]).collect();
        assert_eq!(pares, vec![(0, "a"), (1, "b")]);

        let zipado: Vec<(i32, i32)> = zip(vec![1, 2], vec![10, 20]).collect();
        assert_eq!(zipado, vec![(1, 10), (2, 20)]);

        assert!(any(vec![false, true, false]));
        assert!(all(vec![true, true]));

        let hist = histogram("banana".chars());
        assert_eq!(hist.get(&'a'), Some(&3));
        assert_eq!(hist.get(&'n'), Some(&2));
    }

    #[test]
    fn aritmetica_basica_e_erros_de_borda() {
        use arithmetic::*;
        assert_eq!(sum(vec![1usize, 2, 3]), 6usize);
        assert_eq!(sub(vec![10isize, 1, 2]), Some(7isize));
        assert_eq!(mult(vec![2, 3, 4]), 24);
        assert_eq!(div(10, 2), Ok(5));
        assert_eq!(div(10, 0), Err(ArithError::DivisaoPorZero));
        assert_eq!(divmod(7, 3), Ok((2, 1)));
        assert_eq!(abs(-42i32), 42);
        assert_eq!(pow(2, 10), 1024);
        assert!((root(9.0, 2.0).unwrap() - 3.0).abs() < 1e-9);
        assert!((root(-8.0, 3.0).unwrap() + 2.0).abs() < 1e-9);
        assert_eq!(root(-8.0, 2.5), Err(ArithError::RaizDeNegativoComIndicePar));
        assert_eq!(root(9.0, 0.0), Err(ArithError::IndiceDeRaizInvalido));
    }

    #[test]
    fn strings_utilitarios_completos() {
        use string_utils::*;
        assert_eq!(split("a,b,c", ","), vec!["a", "b", "c"]);
        assert_eq!(strip("  oi  "), "oi");
        assert_eq!(join(", ", vec!["a", "b", "c"]), "a, b, c");
        assert_eq!(join(", ", Vec::<&str>::new()), "");
        assert_eq!(replace("banana", "na", "NA"), "baNANA");
        assert_eq!(lower("OlÁ"), "olá");
        assert_eq!(upper("olá"), "OLÁ");
        assert!(startswith("rust_lang", "rust"));
        assert!(endswith("rust_lang", "lang"));
        assert_eq!(find("café_bom", "bom"), 5);
        assert_eq!(find("abc", "z"), -1);
        assert_eq!(count("banana", "na"), 2);
        assert!(is_palindrome("reviver"));
        assert!(is_palindrome("osso"));
        assert!(!is_palindrome("python"));
    }

    class! {
        struct Contador {
            valor: i32
        }
        impl {
            fn incrementar(&mut self) {
                self.valor += 1;
            }
            fn valor_atual(&self) -> i32 {
                self.valor
            }
        }
    }

    class! {
        struct ContadorComPasso : Contador {
            passo: i32
        }
        impl {
            fn avancar(&mut self) {
                self.valor += self.passo;
            }
        }
    }

    #[test]
    fn class_macro_base_e_heranca_funcionam() {
        let mut c = Contador::new(0);
        c.incrementar();
        c.incrementar();
        assert_eq!(c.valor_atual(), 2);

        // Testa herança estilo C++ (ContadorComPasso : Contador)
        let mut cp = ContadorComPasso::new(Contador::new(10), 5);
        cp.incrementar(); // Método herdado da base via DerefMut!
        cp.avancar();     // Método próprio da filha
        assert_eq!(cp.valor_atual(), 16);
    }
}

#[cfg(test)]
mod tests_tempo_rede_random {
    use super::net_utils::*;
    use super::random_utils::*;
    use super::time_utils::*;

    #[test]
    fn time_e_perf_counter_avancam() {
        assert!(time() > 1.7e9);
        let t0 = perf_counter();
        sleep(0.02);
        assert!(perf_counter() - t0 >= 0.019);
        let n0 = perf_counter_ns();
        assert!(perf_counter_ns() >= n0);
    }

    #[test]
    fn macro_timeit_higienica_funciona() {
        let mut c = 0;
        let t = crate::timeit!(7, {
            c += 1;
        });
        assert_eq!(c, 7);
        assert!(t >= 0.0);
    }

    #[test]
    fn gmtime_e_strftime() {
        let t = gmtime(1_000_000_000);
        assert_eq!(strftime("%Y-%m-%d %H:%M:%S", &t), "2001-09-09 01:46:40");
        assert_eq!(mktime_utc(&t), 1_000_000_000);
    }

    #[test]
    fn tcp_e_udp_loopback() {
        let srv = TcpServer::listen("127.0.0.1:0").unwrap();
        let porta = srv.local_addr().unwrap().port();
        let th = std::thread::spawn(move || {
            let (mut cli, _) = srv.accept().unwrap();
            let d = cli.recv_exact(5).unwrap();
            cli.send(&d).unwrap();
        });
        let mut c = TcpClient::connect(("127.0.0.1", porta)).unwrap();
        c.set_timeout(Some(2.0)).unwrap();
        c.send("olá!").unwrap();
        assert_eq!(c.recv_exact(5).unwrap(), "olá!".as_bytes());
        th.join().unwrap();

        let a = UdpSock::bind("127.0.0.1:0").unwrap();
        let b = UdpSock::bind("127.0.0.1:0").unwrap();
        b.send_to("ping", a.local_addr().unwrap()).unwrap();
        let (msg, _) = a.recv_from(512).unwrap();
        assert_eq!(msg, b"ping");
    }

    #[test]
    fn random_seed_randint_choice_shuffle() {
        seed(42);
        for _ in 0..50 {
            let r = random();
            assert!((0.0..1.0).contains(&r));
            let n = randint(5, 10);
            assert!((5..=10).contains(&n));
        }
        let itens = [10, 20, 30, 40];
        assert!(choice(&itens).is_some());
        let mut v = vec![1, 2, 3, 4, 5, 6, 7, 8];
        shuffle(&mut v);
        assert_eq!(v.len(), 8);
    }
}

#[cfg(test)]
mod tests_fs {
    use super::fs_utils::*;
    use std::fs;

    fn area_de_teste(sufixo: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pyrsutils_v14_fs_{}_{}",
            std::process::id(),
            sufixo
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn io_texto_e_walk_seguro_contra_loop_symlink() {
        let dir = area_de_teste("io_walk");
        let arq = dir.join("notas.txt");
        write_text(&arq, "linha 1\n").unwrap();
        append_text(&arq, "linha 2\n").unwrap();
        assert_eq!(read_text(&arq).unwrap(), "linha 1\nlinha 2\n");
        assert_eq!(readlines(&arq).unwrap(), vec!["linha 1", "linha 2"]);

        #[cfg(unix)]
        {
            // Cria symlink apontando para o próprio diretório pai (ciclo!)
            let link_ciclico = dir.join("loop_infinito");
            symlink(&dir, &link_ciclico).unwrap();
        }

        // O walk não deve entrar em loop infinito porque não segue symlinks
        let mut passos = 0;
        for item in walk(&dir) {
            let _ = item.unwrap();
            passos += 1;
            assert!(passos < 10, "walk entrou em loop infinito com symlink!");
        }
        rmtree(&dir).unwrap();
    }
}
