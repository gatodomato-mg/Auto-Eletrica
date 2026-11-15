# AutoElétrica — estoque

Sistema de gestão de estoque para auto elétrica, desktop e local-first (Rust + egui + SQLite).
Foco no que o nicho tem de diferente: a mesma peça em vários carros, venda por metro,
miudezas em gaveta, custo médio e alerta de estoque mínimo.

## Rodar

```powershell
cargo run --release
```

Banco em `%LOCALAPPDATA%\AutoEletrica\autoeletrica.db`. A variável `AUTOELETRICA_DB`
aponta para outro arquivo.

Testes do motor de estoque: `cargo test --lib`.

## Fase 1

- Cadastro com código interno automático (`AE0001`), EAN, código de fabricante e OEM.
- Aplicação por texto livre (busca expande `2008 a 2012` para casar `2010`) e
  equivalentes pesquisáveis.
- Localização corredor > prateleira > gaveta e foto por caminho de arquivo.
- Unidade UN/M/KG/L com quantidade fracionada (1,5 m de fio) e dinheiro em centavos.
- Entrada com custo médio ponderado, saída com validação, ajuste de inventário,
  histórico por peça, alerta de mínimo e resumo de valor em custo/venda.

Fases seguintes: O.S. com reserva e baixa, PDV, base de troca e garantia de bateria,
importação de XML da NF-e e relatórios (curva ABC, sem giro, lucratividade).

## Backup e exportação

- Ao abrir, uma cópia do banco vai para `backups\` ao lado do `.db` (mantém as 10 últimas).
- Botão **Backup** grava na hora (`VACUUM INTO`, seguro com o app aberto).
- Botão **Exportar CSV** gera o catálogo com `;` e vírgula decimal (Excel pt-BR).

## Versões (git)

```powershell
cd C:\Users\Administrator\Desktop\AutoEletrica
git log --oneline          # ver versões
git diff                   # ver o que mudou desde o último save
git checkout HEAD -- .     # voltar tudo ao último save
git checkout <hash> -- src # voltar só um arquivo/pasta de uma versão antiga
```

Salvar uma versão nova: `powershell -File C:\Users\Administrator\cerebro\bin\versao.ps1 -Pasta . -Mensagem "o que mudou"`.

