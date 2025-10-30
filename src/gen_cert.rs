use std::{fs, net::IpAddr, path::PathBuf};

use rcgen::{
    CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair, KeyUsagePurpose, SanType,
    SerialNumber,
};
use time::OffsetDateTime;

use crate::{
    CA_CERT, CLIENT_CERT, CLIENT_SECRET, SERVER_CERT, SERVER_SECRET, args::GenCertArgs, config_dir,
};

struct CertGenerator {
    output_path: PathBuf,
}

impl CertGenerator {
    fn new(output_path: PathBuf) -> CertGenerator {
        CertGenerator { output_path }
    }

    fn generate_ca(&self) -> Issuer<'_, KeyPair> {
        let mut ca_params = CertificateParams::default();
        ca_params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Constrained(0));
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "Rex Test CA");
        dn.push(DnType::OrganizationName, "Rex Test Org");
        dn.push(DnType::OrganizationalUnitName, "Rex Test Unit");
        dn.push(DnType::StateOrProvinceName, "Test State");
        dn.push(DnType::CountryName, "CN");
        dn.push(DnType::LocalityName, "Somewhere");
        ca_params.distinguished_name = dn;
        ca_params.serial_number = Some(SerialNumber::from(1));
        ca_params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];
        ca_params.not_before = OffsetDateTime::now_utc();
        ca_params.not_after = OffsetDateTime::now_utc() + time::Duration::days(365);

        let ca_keypair = KeyPair::generate().unwrap();
        let root_cert = ca_params.self_signed(&ca_keypair).unwrap();
        fs::write(self.output_path.join(CA_CERT), root_cert.pem()).unwrap();
        Issuer::from_ca_cert_der(root_cert.der(), ca_keypair).unwrap()
    }

    fn generate_server(&self, issuer: &Issuer<KeyPair>) {
        let mut server_params = CertificateParams::default();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "Rex Test Server");
        dn.push(DnType::OrganizationName, "Rex Test Org");
        dn.push(DnType::OrganizationalUnitName, "Rex Test Unit");
        dn.push(DnType::StateOrProvinceName, "Test State");
        dn.push(DnType::CountryName, "CN");
        dn.push(DnType::LocalityName, "Somewhere");
        server_params.distinguished_name = dn;
        server_params.key_usages = [
            KeyUsagePurpose::KeyEncipherment,
            KeyUsagePurpose::DigitalSignature,
        ]
        .into();
        server_params.not_before = OffsetDateTime::now_utc();
        server_params.not_after = OffsetDateTime::now_utc() + time::Duration::days(365);
        server_params.is_ca = IsCa::NoCa;
        server_params.serial_number = Some(SerialNumber::from(2));
        server_params.subject_alt_names = [
            SanType::DnsName("localhost".parse().unwrap()),
            SanType::IpAddress(IpAddr::V4("127.0.0.1".parse().unwrap())),
            SanType::IpAddress(IpAddr::V6("::1".parse().unwrap())),
        ]
        .into();

        let server_keypair = KeyPair::generate().unwrap();
        let server_cert = server_params.signed_by(&server_keypair, issuer).unwrap();
        fs::write(self.output_path.join(SERVER_CERT), server_cert.pem()).unwrap();
        fs::write(
            self.output_path.join(SERVER_SECRET),
            server_keypair.serialize_pem(),
        )
        .unwrap();
    }

    fn generate_client(&self, issuer: &Issuer<KeyPair>) {
        let mut client_params = CertificateParams::default();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "Rex Test Client");
        dn.push(DnType::OrganizationName, "Rex Test Org");
        dn.push(DnType::OrganizationalUnitName, "Rex Test Unit");
        dn.push(DnType::StateOrProvinceName, "Test State");
        dn.push(DnType::CountryName, "CN");
        dn.push(DnType::LocalityName, "Somewhere");
        client_params.distinguished_name = dn;
        client_params.key_usages = [
            KeyUsagePurpose::KeyEncipherment,
            KeyUsagePurpose::DigitalSignature,
        ]
        .into();
        client_params.not_before = OffsetDateTime::now_utc();
        client_params.not_after = OffsetDateTime::now_utc() + time::Duration::days(365);
        client_params.is_ca = IsCa::NoCa;
        client_params.serial_number = Some(SerialNumber::from(3));

        let client_keypair = KeyPair::generate().unwrap();
        let client_cert = client_params.signed_by(&client_keypair, issuer).unwrap();
        fs::write(self.output_path.join(CLIENT_CERT), client_cert.pem()).unwrap();
        fs::write(
            self.output_path.join(CLIENT_SECRET),
            client_keypair.serialize_pem(),
        )
        .unwrap();
    }
}

pub fn gen_cert_main(args: GenCertArgs) {
    let output_path = args.output_path.unwrap_or(config_dir().unwrap());
    fs::create_dir_all(dbg!(&output_path)).unwrap();
    let cg = CertGenerator::new(output_path);
    let issuer = cg.generate_ca();
    cg.generate_server(&issuer);
    cg.generate_client(&issuer);
}
