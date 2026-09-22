# Convenção de Nomenclatura — Releases

## Tags

Todas as tags devem seguir o formato **semver** com prefixo `v`:

```
v0.1.0
v0.2.0
v1.0.0
v1.2.3
```

## Assets

Cada release deve conter os seguintes arquivos como assets (binários anexados):

| Plataforma | Nome do asset | Formato |
|---|---|---|
| Windows x86_64 | `furinar-windows-x86_64.exe` | Executável nativo |
| Linux x86_64 | `furinar-linux-x86_64.tar.gz` | Tarball com binário + SHA256 |
| macOS x86_64 | `furinar-macos-x86_64.tar.gz` | Tarball *(futuro)* |
| macOS aarch64 | `furinar-macos-aarch64.tar.gz` | Tarball *(futuro)* |

### Gerar os assets

```bash
make release
```

Isso gera em `dist/release/`:
- `furinar-windows-x86_64.exe`
- `furinar-linux-x86_64.tar.gz`
- `SHA256SUMS`

Faça upload de todos os arquivos de `dist/release/` como assets da release no GitHub.

### Checklist de uma release

1. Atualizar versão em `Cargo.toml` (`version = "x.y.z"`)
2. Atualizar `CHANGELOG.md` com as mudanças da versão
3. Rodar `make release` (precisa de um build em cada plataforma)
4. Criar a tag: `git tag vX.Y.Z`
5. Criar a release no GitHub com a tag correspondente
6. Copiar o título do CHANGELOG como descrição da release
7. Upload dos assets gerados

### Comandos para fazer push e criar tag

```bash
# 1. Salvar e commitar todas as alterações
git add -A
git commit -m "Bump version to X.Y.Z"

# 2. Push do commit para o master
git push origin master

# 3. Criar a tag
git tag vX.Y.Z

# 4. Push da tag (dispara o GitHub Actions)
git push origin vX.Y.Z
```

> **Importante:** O push da tag (passo 4) é o que dispara o GitHub Actions para
> buildar os binários e criar a release automaticamente.
>
> Para alterar a versão alvo, basta recriar a tag:
> ```bash
> git tag -d vX.Y.Z
> git push origin :refs/tags/vX.Y.Z
> git tag vX.Y.Z
> git push origin vX.Y.Z
> ```

## Auto-update

O mecanismo de auto-update (`src/updates.rs`) usa a API do GitHub Releases para:

1. Buscar a lista de releases
2. Encontrar a release mais recente que contenha um asset com nome correspondente à plataforma
3. Comparar versão semver com a versão compilada no binário
4. Baixar e substituir o binário quando o usuário confirma

**Importante:** O auto-update **não** detecta versões de assets com nomes diferentes. Mantenha a convenção de nomenclatura acima.
