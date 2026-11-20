# Roteiro de teste manual: pareamento e sync Mac ↔ iPhone

Não há automação para isto (o teste de interface `JoinTests` usa um computador de mentira, `fake_mac`).
Faça este roteiro antes de cada versão que mexa em sync, pareamento, QR ou arquivo `.p365sync`.

## Antes de começar
- Mac e iPhone na **mesma rede Wi-Fi** (sem "isolamento de clientes", comum em Wi-Fi de visitantes).
- Desktop: versão a testar já aberta e com o cofre desbloqueado, com alguns itens (tem que ter um login com TOTP e um com nota).
- iPhone: o pareamento é feito por quem **entra** num cofre existente, então o iPhone tem que ser uma **instalação nova** (apagar o app, ou usar um iPhone/simulador limpo). Um iPhone que já tem cofre próprio não entra em outro: hoje não há "juntar" dois cofres.
- Anote: versão do desktop, build do iPhone, modelo do Mac e do iPhone.

## 1. Pareamento por QR code
1. **Mac:** Dispositivos › Adicionar um dispositivo. Aparece o QR code e o texto do código.
2. **iPhone (instalação nova):** "Configurar com outro dispositivo". Permitir câmera e rede local quando pedir.
3. Escanear o QR. Esperado: o iPhone mostra um **código de 6 dígitos**.
4. **Mac:** digitar o código mostrado no iPhone e confirmar "Adicionar dispositivo".
   - Teste negativo (repita o pareamento depois): digitar um código errado → o pareamento é interrompido com a mensagem de código errado e é preciso recomeçar.
5. **iPhone:** digitar a senha mestra do Mac e dar um nome ao aparelho → Concluir. Esperado: os itens do Mac aparecem no iPhone; a Secret Key não é pedida.
6. Verificar no iPhone: um item com TOTP mostra o código; a senha de um login abre com Face ID desligado/ligado; a nota aparece.

Variações:
- **Colar o código** em vez de escanear (Mac: "Copiar código"; iPhone: campo "Código de pareamento").
- **QR vencido:** esperar mais de 5 minutos e tentar → deve falhar com mensagem clara; o código vale uma única vez.
- **Cancelar** no meio (Mac e iPhone) → nenhum dos dois fica meio configurado.

## 2. Sync nos dois sentidos
1. **iPhone:** criar um login novo e editar uma senha de um item existente. Dispositivos › Sincronizar agora.
2. **Mac:** Dispositivos › Sincronizar agora. Esperado: as duas alterações aparecem no Mac; "Última sincronização" atualiza nos dois.
3. **Mac:** favoritar um item, mover outro para "Excluídos recentemente" e adicionar um anexo. Sincronizar de novo pelo iPhone. Esperado: favorito e exclusão chegam; o anexo chega (e abre com a pré-visualização do iOS).
4. **Conflito:** com os dois fora de sincronia, mudar a senha **do mesmo item** nos dois. Sincronizar. Esperado: a mais nova fica, a outra vai para o histórico de senhas, e a tela avisa "tinha duas senhas diferentes".
5. **Pendências:** alterar no iPhone sem sincronizar. No Mac, o aparelho aparece com "N alterações ainda não chegaram".

## 3. Sem rede em comum
1. Mac em outra rede (ou iPhone com Wi-Fi desligado e dados móveis).
2. **Mac:** Dispositivos › Arquivo de sincronização… para o iPhone → gera `.p365sync`.
3. Mandar o arquivo por AirDrop/Arquivos para o iPhone e abrir no Password365. Esperado: "Importado de …: N alterações recebidas".
4. Repetir no sentido contrário (iPhone → Arquivo de sincronização → Mac).

## 4. Remover dispositivo
1. **Mac:** Dispositivos › Remover (o iPhone) e confirmar.
2. **iPhone:** tentar sincronizar. Esperado: recusado.
3. O iPhone mantém a cópia que já tinha, protegida pela senha mestra (mensagem de aviso no Mac diz isso).

## 5. AutoFill depois do pareamento (iPhone)
1. Ajustes do iOS › Senhas › Opções de Senha: só Password365 ligado.
2. Safari em `github.com/login` (ou outro site com item salvo): tocar no campo de usuário → sugestão do Password365 → Face ID → **usuário e senha** preenchidos.
3. Se Face ID estiver desligado nos Ajustes do app, a folha do AutoFill deve pedir para ligar (e mostrar o motivo técnico na linha cinza).

## O que registrar se algo falhar
- Em que passo parou e o texto exato da tela.
- Se o iPhone mostrou o pedido de permissão de **Rede local** (negar essa permissão impede o pareamento).
- Versão do desktop, build do iPhone, e se o Mac usa VPN ou firewall.
