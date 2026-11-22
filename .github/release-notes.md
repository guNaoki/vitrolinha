## Instalar

1. Baixe **`Vitrolinha_*_x64-setup.exe`** abaixo e abra. Instala só para o seu usuário (não pede administrador), cria o atalho no menu Iniciar e traz um desinstalador.
2. Prefere sem instalar? Use o `.zip` portátil.

Requisitos: Windows 10 ou 11 de 64 bits com o WebView2 (já vem no Windows 11; o instalador baixa se faltar). Feito e testado no Windows 11; o Windows 10 deve funcionar, mas ainda não foi testado.

### O Windows mostrou "O Windows protegeu o computador"?

O instalador ainda **não é assinado digitalmente**, então o SmartScreen avisa sobre apps novos. Clique em **Mais informações → Executar assim mesmo**. Para conferir que o arquivo é o mesmo que o GitHub compilou, compare o hash:

```powershell
(Get-FileHash .\Vitrolinha_*_x64-setup.exe).Hash
```

com o de `SHA256SUMS.txt`. O código e o processo de compilação estão neste repositório (`.github/workflows/build.yml`).

---

