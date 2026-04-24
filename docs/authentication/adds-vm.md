# Setting up Active Directory Domain Services (ADDS) on a VM

### Prerequisites

* A hypervisor such as Hyper-V and VirtualBox.

### Steps

1. Download a Windows Server 2025 evaluation ISO image from: https://www.microsoft.com/en-us/evalcenter/evaluate-windows-server-2025.
2. Create a new VM with the above image (2GB RAM, 100GB Disk is enough).
    * Choose **Windows Server 2025 Standard Evaluation (Desktop Experience)**
3. Add **Active Directory Domain Services** role and **Active Directory Certificate Services** role to the server.
4. Promote the server to a domain controller:
   * Select "Add a new forest" with root domain name **corp.local**

   (reboot to join the domain)
5. Finish Certificate Service configuration:
    * Select "Certificate Authority" for role services to configure
    * Set "corp-ca" as the common name
6. Launch Manage computer certificate (certlm)
7. Right-click "Personal"->"Certificates" and select "All Tasks"->"Request New Certificate"
8. Select "Domain Controller Authentication" and click on "Enroll"

   (reboot)

9. Export the certificate in step 8 to a .CER file, i.e. **dc.cer**. Copy to your local machine
10. On your local machine, convert the certificate to .CRT:
    ```bash
    openssl x509 -inform der -in dc.cer -out dc.crt
    ```



