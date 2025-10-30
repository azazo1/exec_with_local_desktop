use std::net::IpAddr;

use exec_with_local_desktop::config_dir;
use rcgen::{
    Certificate, CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair,
    KeyUsagePurpose, SanType, SerialNumber,
};
use time::OffsetDateTime;

#[test]
fn rcgen_ca() {
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
        rcgen::KeyUsagePurpose::KeyCertSign,
        rcgen::KeyUsagePurpose::CrlSign,
        rcgen::KeyUsagePurpose::DigitalSignature,
        rcgen::KeyUsagePurpose::KeyEncipherment,
    ];
    ca_params.not_before = OffsetDateTime::now_utc();
    ca_params.not_after = OffsetDateTime::now_utc() + time::Duration::days(365);

    let ca_keypair = KeyPair::generate().unwrap();
    let root_cert = ca_params.self_signed(&ca_keypair).unwrap();
    println!("root cert:\n{}", root_cert.pem());
    println!("root cert secret:\n{}", ca_keypair.serialize_pem());
}

#[test]
fn rcgen_server() {
    let ca_params = CertificateParams::default();
    let ca_keypair = KeyPair::generate().unwrap();
    let issuer = Issuer::from_params(&ca_params, ca_keypair);

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
    let server_cert = server_params.signed_by(&server_keypair, &issuer).unwrap();
    println!("server cert:\n{}", server_cert.pem());
    println!("server secret:\n{}", server_keypair.serialize_pem());
}

#[test]
fn rcgen_client() {
    let ca_params = CertificateParams::default();
    let ca_keypair = KeyPair::generate().unwrap();
    let issuer = Issuer::from_params(&ca_params, ca_keypair);

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
    let client_cert = client_params.signed_by(&client_keypair, &issuer).unwrap();
    println!("client cert:\n{}", client_cert.pem());
    println!("client secret:\n{}", client_keypair.serialize_pem());
}

#[test]
fn dirs() {
    println!("{:?}", config_dir().unwrap());
}

#[test]
fn tls_grpc() {
    // Server::builder().tls_config(ServerTlsConfig::new().client_ca_root())
}
