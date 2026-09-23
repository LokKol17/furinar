# Convenção de Versão e Release

## Versão Única de Verdade

A versão do projeto é definida em **`Cargo.toml`**. Todos os arquivos de packaging
devem refletir essa mesma versão.

| Arquivo | Campo | Exemplo |
|---|---|---|
| `Cargo.toml` | `version` | `1.5.2` |
| `pkg/debian/control` | `Version:` | `Version: 1.5.2` |
| `pkg/arch/PKGBUILD` | `pkgver=` | `pkgver=1.5.2` |
| `pkg/fedora/furinar.spec` | `Version:` | `Version:        1.5.2` |
| `pkg/install.sh` | `VERSION=` | `VERSION="${FURINAR_VERSION:-1.5.2}"` |
| `pkg/furinar.metainfo.xml` | `<release>` | `<release version="1.5.2">` |

O Makefile também lê a versão automaticamente do `Cargo.toml` (via `grep` + `sed`).

---

## Tags

Todas as tags devem seguir o formato **semver** com prefixo `v`:

```
v0.1.0
v0.2.0
v1.0.0
v1.2.3
```

---

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

---

## Workflow de Release

### 1. Atualizar a versão no Cargo.toml

Edite `Cargo.toml` e mude a linha `version = "X.Y.Z"` para a nova versão.

```toml
[package]
version = "1.6.0"
```

### 2. Sincronizar todas as versões

Rode o comando que atualiza automaticamente todos os arquivos de packaging:

```bash
make sync-version
```

Isso atualiza:
- `pkg/debian/control`
- `pkg/arch/PKGBUILD`
- `pkg/fedora/furinar.spec`
- `pkg/install.sh`
- `pkg/furinar.metainfo.xml`

### 3. Verificar que está tudo em sync

```bash
make check-version
```

Se tudo estiver correto, vai mostrar:
```
  All packaging versions match: 1.6.0
```

Se houver algum desatualizado, vai apontar qual arquivo está errado.

### 4. Atualizar o CHANGELOG

Adicione uma entrada em `CHANGELOG.md` com as mudanças da versão.

### 5. Build e Release

```bash
make build
make release
make deb   # opcional
make rpm   # opcional
```

Os artefatos gerados ficam em `dist/`.

### Checklist de uma release

1. Atualizar versão em `Cargo.toml` (`version = "x.y.z"`)
2. Rodar `make sync-version` para propagar a versão aos arquivos de packaging
3. Rodar `make check-version` para confirmar que está tudo em sync
4. Atualizar `CHANGELOG.md` com as mudanças da versão
5. Rodar `make release` (precisa de um build em cada plataforma)
6. Criar a tag: `git tag vX.Y.Z`
7. Criar a release no GitHub com a tag correspondente
8. Copiar o título do CHANGELOG como descrição da release
9. Upload dos assets gerados

---

## Comandos para fazer push e criar tag

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

---

## Auto-update

O mecanismo de auto-update (`src/updates.rs`) usa a API do GitHub Releases para:

1. Buscar a lista de releases
2. Encontrar a release mais recente que contenha um asset com nome correspondente à plataforma
3. Comparar versão semver com a versão compilada no binário
4. Baixar e substituir o binário quando o usuário confirma

**Importante:** O auto-update **não** detecta versões de assets com nomes diferentes. Mantenha a convenção de nomenclatura acima.

---

## Regras

1. **A fonte da verdade é `Cargo.toml`** — nunca mude a versão em outro arquivo sem antes mudar no `Cargo.toml` e rodar `make sync-version`.

2. **Use SemVer** — [Semantic Versioning](https://semver.org/): `MAJOR.MINOR.PATCH`
   - `MAJOR`: mudanças incompatíveis
   - `MINOR`: funcionalidades novas (compatível)
   - `PATCH`: correções de bug (compatível)

3. **Não esqueça de atualizar o CHANGELOG** — toda versão nova deve ter uma entrada.

4. **O self-updater do Furinar** (no `src/updates.rs`) compara a versão compilada com o
   `CARGO_PKG_VERSION` do `Cargo.toml`. Se as versões de packaging estiverem desatualizadas,
   o updater pode oferecer uma atualização incorreta.

5. **Tags no Git** devem seguir o formato `vX.Y.Z` (ex: `v1.6.0`, não `1.6.0`).

---

## Comandos Rápidos

```bash
make help           # Mostra todos os comandos disponíveis
make sync-version   # Sincroniza versão com Cargo.toml
make check-version  # Verifica se versões estão em sync
make release        # Gera artefatos para GitHub Releases
```
