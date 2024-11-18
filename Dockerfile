# first stage downloads the full NCIT vocabulary list from nci.nih.gov
FROM alpine:latest as ncit-getter
# get programs needed to copy and unzip ncit
RUN apk add unzip curl
# make a directory and unzip the thesaurus into it
RUN mkdir -p /opt/ ; \
  curl -L https://evs.nci.nih.gov/ftp1/NCI_Thesaurus/Thesaurus.FLAT.zip -o /opt/thesaurus.zip ; \
  unzip /opt/thesaurus.zip -d /opt

# second stage runs the rust code to add new terms w/ the NCIT
# preferred term as the display value and the PQCMC preferred
# term as a synonym
FROM rust:1.82 AS build
COPY . .
COPY --from=ncit-getter /opt/Thesaurus.txt .
ENV NEW_CODES="new-codes.json"
ENV THESAURUS="Thesaurus.txt"
RUN cargo run > warnings.txt

# third stage copies the output into the current directory
FROM scratch
COPY --from=build ./output.json /
COPY --from=build ./warnings.txt /