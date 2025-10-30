use std::{fs, net::IpAddr, thread, time::Duration};

use exec_with_local_desktop::{
    CA_CERT, CLIENT_CERT, CLIENT_SECRET, SERVER_CERT, SERVER_SECRET,
    client::{ExecuteOptions, ExecutorClient},
    config_dir,
    exec::{
        Command, ExecuteRequestChunk, execute_client::ExecuteClient,
        execute_request_chunk::RequestChunk, execute_server::ExecuteServer,
    },
    server::Executor,
};
use rcgen::{
    CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair, KeyUsagePurpose, SanType,
    SerialNumber,
};
use time::OffsetDateTime;
use tonic::transport::{Channel, Endpoint, Identity, Server, ServerTlsConfig};
use tracing::{Level, info};

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

fn tls_server(ca: tonic::transport::Certificate) {
    let addr = "0.0.0.0:23845".parse().unwrap();
    let config_dir = config_dir().unwrap();
    let server_cert = fs::read(config_dir.join(SERVER_CERT)).unwrap();
    let server_secret = fs::read(config_dir.join(SERVER_SECRET)).unwrap();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async move {
        Server::builder()
            .tls_config(
                ServerTlsConfig::new()
                    .client_ca_root(ca)
                    .identity(Identity::from_pem(server_cert, server_secret)),
            )
            .unwrap()
            .add_service(ExecuteServer::new(Executor))
            .serve(addr)
            .await
            .unwrap();
    });
    info!("server ended");
}

fn tls_client(ca: tonic::transport::Certificate) {
    let addr = "https://localhost:23845"; // 这里不能够使用 grpc:// 作为 schema, 必须使用 https!!!
    let config_dir = config_dir().unwrap();
    let client_cert = fs::read(config_dir.join(CLIENT_CERT)).unwrap();
    let client_secret = fs::read(config_dir.join(CLIENT_SECRET)).unwrap();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async move {
        let tls_config = tonic::transport::ClientTlsConfig::new()
            .ca_certificate(ca)
            .domain_name("localhost")
            .identity(Identity::from_pem(client_cert, client_secret));
        let chan = Channel::from_static(addr)
            .tls_config(tls_config)
            .unwrap()
            .connect()
            .await
            .unwrap();
        let mut client = ExecuteClient::new(chan);
        let rst = client
            .execute(tokio_stream::once(ExecuteRequestChunk {
                request_chunk: Some(RequestChunk::Command(Command {
                    executable: "bash".into(),
                    args: ["-c".into(), "ls".into()].into(),
                    current_dir: None,
                    leak: false,
                })),
            }))
            .await
            .unwrap();
        dbg!(rst.into_inner().message().await.unwrap());
    });
    info!("client ended");
}

#[test]
fn tls_grpc() {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();
    use tonic::transport::Certificate;
    let config_dir = config_dir().unwrap();
    let ca = Certificate::from_pem(fs::read(config_dir.join(CA_CERT)).unwrap());
    let ca1 = ca.clone();
    let s_join = thread::spawn(move || {
        tls_server(ca1);
    });
    thread::sleep(Duration::from_secs(1));
    let c_join = thread::spawn(move || {
        tls_client(ca);
    });
    c_join.join().unwrap();
    s_join.join().unwrap();
}
